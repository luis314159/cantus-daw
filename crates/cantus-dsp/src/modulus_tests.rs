//! Modulus's DSP, held to the research's test plan (§9): the decimator's numbers, the DX
//! envelope against the published formulas, the guard's estimate against measured spectra,
//! feedback stability and the anti-hunting zero, and block-size independence.

use super::env::*;
use super::guard::{bandwidth, graph};
use super::tables::{design_halfband, tables, Decimator, HB_COEFS, HB_TRANSITION};
use super::*;
use crate::spectrum::fft;

const FS: f32 = 48_000.0;

/// Amplitude of a sine through a 2:1 half-band, from its RMS after the transient.
fn through_halfband(freq_in_over_fs_in: f64) -> f32 {
    let c = design_halfband(HB_COEFS, HB_TRANSITION);
    let mut d = Decimator::default();
    let (mut sum, mut count) = (0f64, 0usize);
    for i in 0..8000 {
        let mut buf = [0f32; 8];
        for (k, b) in buf.iter_mut().take(2).enumerate() {
            let n = (2 * i + k) as f64;
            *b = (2.0 * std::f64::consts::PI * freq_in_over_fs_in * n).sin() as f32;
        }
        let y = d.process(&c, &mut buf, 2);
        if i > 2000 {
            sum += (y as f64) * (y as f64);
            count += 1;
        }
    }
    (2.0 * sum / count as f64).sqrt() as f32
}

#[test]
fn the_half_band_passes_the_band_and_stops_what_would_fold() {
    // Pass band: up to 0.45 of the output's Nyquist (0.225 of the input rate).
    for f in [0.01, 0.1, 0.2, 0.22] {
        let a = through_halfband(f);
        assert!((a - 1.0).abs() < 0.01, "pass band at {f}: {a}");
    }
    // Stop band: from 0.55 of the output's Nyquist; everything there would fold.
    for f in [0.28, 0.3, 0.4, 0.49] {
        let a = through_halfband(f);
        assert!(
            a < 1e-4,
            "stop band at {f}: {a} ({:.0} dB)",
            20.0 * a.log10()
        );
    }
}

#[test]
fn the_dx_level_tables_match_the_published_mapping() {
    assert_eq!(dx_scale_level(99), 127);
    assert_eq!(dx_scale_level(20), 48);
    assert_eq!(dx_scale_level(19), 46);
    assert_eq!(dx_scale_level(0), 0);
    assert_eq!(dx_env_steps(99), DX_FULL);
    // "L 0–5 → 2L; 5–16 → 5 + L; 17–20 → 4 + L; 20–99 → 14 + L/2", times 64.
    for (l, lp) in [(3u8, 6u32), (10, 15), (18, 22), (60, 44)] {
        assert_eq!(dx_env_steps(l), (64 * lp) as f32 - 192.0, "L {l}");
    }
    assert_eq!(dx_output_level_db(99), 0.0);
    assert!(
        (dx_output_level_db(98) + 0.7526).abs() < 1e-3,
        "0.75 dB per step"
    );
    assert_eq!(dx_amp(DX_FLOOR), 0.0, "the floor is silence");
    assert!((dx_amp(DX_FULL) - 1.0).abs() < 1e-6);
}

#[test]
fn dx_decay_is_linear_in_db_at_the_published_speed() {
    for rate in [20u8, 40, 60, 80] {
        let q = dx_qrate(rate, 0);
        let s = DxEnvSettings {
            rates: [99, rate, 99, 99],
            levels: [99, 0, 0, 0],
        };
        let mut e = DxEnv::default();
        e.start(&s, 0.0, 0, false);
        while e.stage == 0 {
            e.advance(&s, 1e-4); // the attack, to L1
        }
        assert_eq!(e.stage, 1, "rate {rate}");
        let a = e.level;
        let dt = 0.001;
        e.advance(&s, dt);
        let measured = (a - e.level) * DX_STEP_DB / dt;
        let published = 0.2819 * (1u32 << (q >> 2)) as f32 * (1.0 + 0.25 * (q & 3) as f32);
        assert!(
            (measured / published - 1.0).abs() < 0.01,
            "rate {rate} (qrate {q}): {measured} dB/s against {published}"
        );
    }
}

#[test]
fn dx_attack_jumps_forty_db_then_climbs_concave() {
    let s = DxEnvSettings {
        rates: [50, 99, 99, 99],
        levels: [99, 99, 99, 0],
    };
    let mut e = DxEnv::default();
    e.start(&s, 0.0, 0, false);
    assert_eq!(e.level, DX_FLOOR);
    e.advance(&s, 1e-6);
    assert!(e.level >= 1716.0, "the jump: {}", e.level);
    // Faster (in dB) low down than near the top.
    let low = {
        let before = e.level;
        e.advance(&s, 0.002);
        e.level - before
    };
    while e.level < 3500.0 {
        e.advance(&s, 0.002);
    }
    let before = e.level;
    e.advance(&s, 0.002);
    let high = e.level - before;
    assert!(low > 2.0 * high, "concave attack: {low} then {high}");
}

#[test]
fn a_breakpoint_envelope_holds_loops_and_releases() {
    let mut s = TimeEnvSettings::default();
    let mut e = TimeEnv::default();
    e.start(&s, [1.0; 3], false);
    e.advance(&s, 0.005);
    assert!((e.level - 1.0).abs() < 1e-5, "attack reached: {}", e.level);
    e.advance(&s, 2.0);
    assert!((e.level - 0.7).abs() < 1e-5, "holds the sustain point");
    e.release(&s);
    e.advance(&s, 0.25);
    assert!(e.level.abs() < 1e-6 && e.done(&s));
    // A loop between point 1 and the sustain point keeps moving.
    s.n = 4;
    s.l[..4].copy_from_slice(&[1.0, 0.2, 1.0, 0.0]);
    s.t[..4].copy_from_slice(&[0.01, 0.01, 0.01, 0.1]);
    s.sus = 3;
    s.lp = 1;
    e.start(&s, [1.0; 3], false);
    let mut seen = (1f32, 0f32);
    for _ in 0..400 {
        let l = e.advance(&s, 0.0005);
        seen = (seen.0.min(l), seen.1.max(l));
    }
    assert!(seen.0 < 0.3 && seen.1 > 0.9, "loops: {seen:?}");
}

// ---- the whole synth ---------------------------------------------------------------------

/// Render one note of `p` for `secs` (held for `hold`), at `block` samples per call.
pub(super) fn render(p: &Patch, key: u8, hold: f32, secs: f32, block: usize) -> Vec<f32> {
    let mut m = Modulus::new(FS);
    m.set_patch(p);
    let n = (secs * FS) as usize;
    let off = (hold * FS) as usize;
    let mut out = vec![0f32; n];
    let mut r = vec![0f32; n];
    m.note_on(key, 1, 1.0);
    let mut i = 0;
    while i < n {
        let end = (i + block).min(n);
        let end = if i < off && end > off { off } else { end };
        if i == off {
            m.note_off(key, 1, 0.0);
        }
        m.process(&mut out[i..end], &mut r[i..end]);
        i = end;
    }
    out
}

/// Two operators: B (ratio `ratio`) into A with `index` radians, A heard.
pub(super) fn pair(ratio: f32, index: f32) -> Patch {
    let mut p = Patch::default();
    p.ops[1].ratio = ratio;
    p.m[1][0] = index;
    for o in &mut p.ops {
        o.time.l[1] = 1.0; // hold full level
    }
    p
}

#[test]
fn the_default_patch_is_a_sine_at_the_key() {
    let out = render(&Patch::default(), 69, 1.0, 1.0, 256);
    let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
    assert!((peak - VOICE_SCALE).abs() < 0.01, "peak {peak}");
    // Zero crossings over half a second: 440 Hz.
    let seg = &out[12_000..36_000];
    let ups = seg.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
    assert!((ups as i32 - 220).abs() <= 1, "{ups} cycles in 0.5 s");
}

#[test]
fn every_block_size_renders_the_same_samples() {
    let mut p = pair(3.5, 4.0);
    p.ops[2].ratio = 1.0;
    p.ops[2].fb = 1.2;
    p.ops[2].out_db = -6.0;
    p.lfo1_pitch = 30.0;
    p.quality = 3; // 4×
    let reference = render(&p, 64, 0.3, 0.6, 512);
    for block in [1usize, 7, 64, 1000] {
        assert_eq!(reference, render(&p, 64, 0.3, 0.6, block), "block {block}");
    }
}

#[test]
fn the_voice_ends_in_exact_silence() {
    let out = render(&pair(2.0, 3.0), 60, 0.2, 1.0, 128);
    assert!(out[..9_600].iter().any(|x| *x != 0.0));
    assert!(
        out[40_000..].iter().all(|x| *x == 0.0),
        "exact zeros after the release"
    );
}

/// The frequency under which 99 % of the energy lies, from a long FFT.
fn energy_edge(x: &[f32], fs: f32) -> f32 {
    let n = x.len().next_power_of_two() / 2;
    let mut re: Vec<f32> = x[..n]
        .iter()
        .enumerate()
        .map(|(i, v)| v * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos()))
        .collect();
    let mut im = vec![0f32; n];
    fft(&mut re, &mut im);
    let p: Vec<f32> = (0..n / 2).map(|k| re[k] * re[k] + im[k] * im[k]).collect();
    let total: f32 = p.iter().sum();
    let mut acc = 0.0;
    for (k, v) in p.iter().enumerate() {
        acc += v;
        if acc >= 0.99 * total {
            return k as f32 * fs / n as f32;
        }
    }
    fs / 2.0
}

#[test]
fn carsons_estimate_covers_the_measured_energy() {
    for (ratio, index) in [(1.0, 1.0), (1.0, 5.0), (2.0, 3.0), (3.5, 2.0), (0.5, 8.0)] {
        let mut p = pair(ratio, index);
        p.quality = 4; // 8×: nothing folds, the spectrum is the real one
        let out = render(&p, 57, 1.0, 0.5, 256);
        let measured = energy_edge(&out[4_096..], FS);
        let g = graph(&p);
        let hz = [220.0, 220.0 * ratio, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let peak = [1.0; OPS];
        let predicted = bandwidth(&p, &g, &hz, &peak, 1.0);
        assert!(
            predicted * 1.01 >= measured,
            "ratio {ratio}, index {index}: predicted {predicted} Hz, measured {measured} Hz"
        );
    }
}

#[test]
fn feedback_stays_bounded_and_the_average_removes_the_nyquist_mode() {
    let top = |avg: bool, beta: f32| {
        let mut p = Patch::default();
        p.ops[0].fb = beta;
        p.ops[0].time.l[1] = 1.0;
        p.fb_avg = avg;
        p.quality = 0;
        let out = render(&p, 84, 1.0, 0.4, 256);
        assert!(out
            .iter()
            .all(|x| x.is_finite() && x.abs() <= VOICE_SCALE * 1.001));
        // Energy in the top 1 % of the band against the total.
        let seg = &out[8_192..8_192 + 8_192];
        let mut re: Vec<f32> = seg.to_vec();
        let mut im = vec![0f32; re.len()];
        fft(&mut re, &mut im);
        let n = re.len();
        let p: Vec<f32> = (0..n / 2).map(|k| re[k] * re[k] + im[k] * im[k]).collect();
        let total: f32 = p.iter().sum();
        let hi: f32 = p[(n / 2) * 99 / 100..].iter().sum();
        10.0 * (hi / total).max(1e-20).log10()
    };
    for b in 0..=40 {
        top(true, b as f32 * 0.1);
    }
    // Where the plain one-sample loop hunts (β 1.2…2, the limit cycle at Nyquist), the
    // two-sample average keeps the top of the band empty.
    for beta in [1.5f32, 2.0] {
        let (with, without) = (top(true, beta), top(false, beta));
        assert!(
            with + 20.0 <= without,
            "β {beta}: top-band energy {with:.1} dB with the average, {without:.1} dB without"
        );
    }
}

#[test]
fn the_tables_are_shared_and_exact_at_the_quadrants() {
    let t = tables();
    assert_eq!(t.sin(0), 0.0);
    assert!((t.sin(1 << 30) - 1.0).abs() < 1e-6);
    assert!((t.sin(3 << 30) + 1.0).abs() < 1e-6);
    assert!(Arc::ptr_eq(&t, &tables()));
}

/// CPU per voice (a bench, not a check): one voice of eight operators — two stacks of four,
/// feedback on the top — held for ten seconds at each quality. `cargo test -p cantus-dsp
/// --release modulus_speed -- --ignored --nocapture --test-threads=1`.
#[test]
#[ignore]
fn modulus_speed() {
    let mut p = Patch::default();
    for (s, t) in [(7, 6), (6, 5), (5, 4), (3, 2), (2, 1), (1, 0)] {
        p.m[s][t] = 1.5;
    }
    p.ops[4].out_db = 0.0;
    p.ops[7].fb = 0.5;
    for o in &mut p.ops {
        o.time.l[1] = 1.0;
    }
    for q in 0..=4u8 {
        p.quality = q;
        let secs = 10.0;
        let start = std::time::Instant::now();
        let out = render(&p, 57, secs, secs, 256);
        let took = start.elapsed().as_secs_f64();
        assert!(out.iter().any(|x| *x != 0.0));
        println!(
            "quality {q}: {:.1} ns per sample, {:.3} % of a core per voice",
            took * 1e9 / (secs as f64 * FS as f64),
            took / secs as f64 * 100.0
        );
    }
}

use std::sync::Arc;
