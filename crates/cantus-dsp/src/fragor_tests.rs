//! Fragor's tests: the null, block-size independence, rest, the footswitch and the chain,
//! the gate, the dying battery, aliasing, calibration and CPU.

use super::*;

const SR: f32 = 48_000.0;

/// A deterministic hash in −1 … 1.
fn hash(i: u64) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

/// A plucked string (Karplus–Strong, its burst a hash): a guitar-like test note, peak
/// `peak`.
pub(super) fn pluck(hz: f32, secs: f32, peak: f32, seed: u64) -> Vec<f32> {
    let n = (secs * SR) as usize;
    let len = (SR / hz) as usize;
    let mut line: Vec<f32> = (0..len)
        .map(|i| hash(seed * 1_000_003 + i as u64))
        .collect();
    // Soften the burst (a pick, not a scrape).
    for _ in 0..2 {
        for i in 1..len {
            line[i] = 0.5 * (line[i] + line[i - 1]);
        }
    }
    let mut out = Vec::with_capacity(n);
    let mut p = 0;
    for _ in 0..n {
        let a = line[p];
        let b = line[(p + 1) % len];
        line[p] = 0.4985 * (a + b);
        out.push(a);
        p = (p + 1) % len;
    }
    let m = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    out.iter().map(|v| v * peak / m).collect()
}

fn noise(n: usize, seed: u64, amp: f32) -> Vec<f32> {
    (0..n).map(|i| amp * hash(seed + i as u64)).collect()
}

fn run(f: &mut Fragor, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (l.to_vec(), r.to_vec());
    let mut i = 0;
    while i < l.len() {
        let e = (i + block).min(l.len());
        f.process(&mut l[i..e], &mut r[i..e]);
        i = e;
    }
    (l, r)
}

fn bits_eq(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn board(chain: [Option<Model>; 3]) -> BoardSettings {
    BoardSettings {
        chain,
        ..BoardSettings::default()
    }
}

fn fresh(s: &BoardSettings) -> Fragor {
    let mut f = Fragor::new(SR);
    f.set(s);
    f.reset();
    f
}

/// The null rule: with the defaults (no pedal on the board, the gate off) every sample
/// comes out as it went in, to the bit; with pedals on the board but every footswitch off,
/// as it went in delayed by the latency the host compensates (a footswitch never moves it).
#[test]
fn the_defaults_and_an_empty_board_touch_no_sample() {
    let x = noise(4096, 1, 0.8);
    let y = noise(4096, 2, 0.8);
    let mut f = Fragor::new(SR);
    let (a, b) = run(&mut f, &x, &y, 64);
    assert!(bits_eq(&a, &x) && bits_eq(&b, &y));
    assert_eq!(f.latency(), 0);
    let mut s = board([Some(Model::Fuzz), Some(Model::Muff), None]);
    s.on = [false; PEDALS];
    let mut f = fresh(&s);
    let lat = f.latency();
    assert!(lat > 0);
    let (a, b) = run(&mut f, &x, &y, 64);
    let n = x.len() - lat;
    assert!(bits_eq(&a[lat..], &x[..n]) && bits_eq(&b[lat..], &y[..n]));
}

/// At minimum drive a pedal still sounds (only an empty board is a null).
#[test]
fn every_pedal_sounds_even_at_its_minimum() {
    for m in Model::ALL {
        let mut s = board([Some(m), None, None]);
        s.knobs[m.index()] = [0.0, 0.5, 1.0];
        let mut f = fresh(&s);
        let x = pluck(196.0, 0.5, 0.3, 3);
        let (a, _) = run(&mut f, &x, &x, 256);
        let e: f32 = a.iter().map(|v| v * v).sum();
        assert!(e > 1e-3, "{m:?} silent: {e}");
        assert!(!bits_eq(&a, &x));
    }
}

/// The output does not depend on the block size (1, 37, 64, 512), with knobs moving, a
/// footswitch, a new order, mono and the gate in the middle of the stream.
#[test]
fn the_block_size_changes_nothing() {
    let x = pluck(110.0, 0.6, 0.4, 7);
    let y = pluck(165.0, 0.6, 0.3, 8);
    let render = |block: usize| {
        let mut s = board([Some(Model::Overdrive), Some(Model::Fuzz), Some(Model::Muff)]);
        s.gate_db = -50.0;
        let mut f = fresh(&s);
        let half = x.len() / 2;
        let (mut a, mut b) = run(&mut f, &x[..half / 2], &y[..half / 2], block);
        s.knobs[Model::Fuzz.index()] = [0.9, 0.3, 0.7];
        s.on[Model::Overdrive.index()] = false;
        f.set(&s);
        let (c, d) = run(&mut f, &x[half / 2..half], &y[half / 2..half], block);
        s.chain = [Some(Model::Muff), Some(Model::Silicon), Some(Model::Fuzz)];
        s.mono = true;
        f.set(&s);
        let (e, g) = run(&mut f, &x[half..], &y[half..], block);
        a.extend(c);
        a.extend(e);
        b.extend(d);
        b.extend(g);
        (a, b)
    };
    let (l1, r1) = render(1);
    for block in [37, 64, 512] {
        let (l, r) = render(block);
        assert!(bits_eq(&l, &l1) && bits_eq(&r, &r1), "block {block}");
    }
}

/// After a second of silence the unit rests, and from there it renders exactly what a new
/// unit with the same settings renders (that one skipped as far, so their grids agree).
#[test]
fn it_rests_after_silence_as_good_as_new() {
    let mut s = board([Some(Model::OpAmp), Some(Model::Fuzz), None]);
    s.gate_db = -60.0;
    let mut f = fresh(&s);
    let x = pluck(220.0, 0.5, 0.5, 11);
    run(&mut f, &x, &x, 128);
    assert!(!f.at_rest());
    let quiet = vec![0.0; (SR * SETTLE_S) as usize + 10];
    let (a, b) = run(&mut f, &quiet, &quiet, 128);
    assert_eq!(f.rest_blocker(), None);
    assert!(a[a.len() - 10..]
        .iter()
        .chain(&b[b.len() - 10..])
        .all(|v| v.to_bits() == 0));
    let mut g = fresh(&s);
    g.skip(x.len() + quiet.len());
    assert!(g.at_rest());
    let z = pluck(147.0, 0.3, 0.4, 12);
    let (p, q) = run(&mut f, &z, &z, 64);
    let (u, v) = run(&mut g, &z, &z, 64);
    assert!(bits_eq(&p, &u) && bits_eq(&q, &v));
}

/// A new unit with pedals engaged gives exact silence for silence, and stays at rest.
#[test]
fn silence_stays_silent() {
    let mut f = fresh(&board([Some(Model::Muff), Some(Model::Silicon), None]));
    assert!(f.at_rest());
    let quiet = vec![0.0; 3000];
    let (a, b) = run(&mut f, &quiet, &quiet, 100);
    assert!(a.iter().chain(&b).all(|v| v.to_bits() == 0));
    assert!(f.at_rest());
}

/// The chain's order is the signal's order.
#[test]
fn the_order_of_the_chain_is_heard() {
    let x = pluck(196.0, 0.4, 0.3, 5);
    let render = |chain| {
        let mut f = fresh(&board(chain));
        run(&mut f, &x, &x, 256).0
    };
    let a = render([Some(Model::Overdrive), Some(Model::Fuzz), None]);
    let b = render([Some(Model::Fuzz), Some(Model::Overdrive), None]);
    let d: f32 = a.iter().zip(&b).map(|(p, q)| (p - q).abs()).sum::<f32>() / a.len() as f32;
    assert!(d > 1e-3, "{d}");
}

/// Mono runs one circuit on the sum and gives it to both sides.
#[test]
fn mono_is_one_circuit_for_both_sides() {
    let mut s = board([Some(Model::Overdrive), None, None]);
    s.mono = true;
    let mut f = fresh(&s);
    let (a, b) = run(
        &mut f,
        &pluck(196.0, 0.3, 0.3, 1),
        &pluck(247.0, 0.3, 0.3, 2),
        64,
    );
    assert!(bits_eq(&a, &b));
}

/// With the mix at 0 the output is the input delayed by the latency exactly (the dry path
/// lines up with the oversampled wet one).
#[test]
fn the_dry_path_is_the_input_delayed_by_the_latency() {
    for os in FACTORS {
        let mut s = board([Some(Model::OpAmp), None, None]);
        s.mix = 0.0;
        s.oversampling = os;
        let mut f = fresh(&s);
        let lat = f.latency();
        assert_eq!(lat, crate::fervor::os::latency(os, false));
        let x = noise(2000, 9, 0.5);
        let (a, _) = run(&mut f, &x, &x, 64);
        assert!(bits_eq(&a[lat..], &x[..x.len() - lat]), "{os}x");
    }
}

/// A footswitch pressed in the middle of a note crossfades: no step larger than the
/// pedal's own largest step.
#[test]
fn a_footswitch_does_not_click() {
    let n = 24_000;
    let w = std::f32::consts::TAU * 110.0 / SR;
    let x: Vec<f32> = (0..n).map(|i| 0.3 * crate::math::sin(w * i as f32)).collect();
    let mut s = board([Some(Model::Silicon), None, None]);
    let mut f = fresh(&s);
    let (a, _) = run(&mut f, &x[..n / 2], &x[..n / 2], 64);
    let max_step = a
        .windows(2)
        .skip(n / 4)
        .map(|p| (p[1] - p[0]).abs())
        .fold(0.0f32, f32::max);
    s.on[Model::Silicon.index()] = false;
    f.set(&s);
    let (b, _) = run(&mut f, &x[n / 2..], &x[n / 2..], 64);
    let joined: Vec<f32> = a.iter().chain(&b).copied().collect();
    let worst = joined[n / 2 - 10..n / 2 + 2000]
        .windows(2)
        .map(|p| (p[1] - p[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(worst <= max_step * 1.05, "{worst} > {max_step}");
}

/// The gate before the pedals silences what falls below its threshold: a fuzz on a
/// −70 dBFS hiss is exact silence once the gate has closed, and loud without the gate.
#[test]
fn the_gate_closes_on_hiss() {
    let mut s = board([Some(Model::Fuzz), None, None]);
    s.knobs[Model::Fuzz.index()] = [1.0, 1.0, 0.5];
    s.gate_db = -50.0;
    let mut f = fresh(&s);
    let mut x = pluck(110.0, 0.5, 0.4, 3);
    x.extend(noise(144_000, 77, 0.0003));
    let (a, _) = run(&mut f, &x, &x, 64);
    // Closed, the gate feeds the pedals silence: a second later they rest, and the output
    // is exact silence.
    let tail = &a[a.len() - 4000..];
    assert!(tail.iter().all(|v| v.to_bits() == 0), "gated tail");
    s.gate_db = gate::OFF_DB;
    let mut g = fresh(&s);
    let (b, _) = run(&mut g, &x, &x, 64);
    let tail = &b[b.len() - 4000..];
    let e: f32 = tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32;
    assert!(e > 1e-6, "ungated tail energy {e}");
}

/// Window RMS levels (dB) of a pedal's output for a decaying note, one window per 0.1 s.
fn decay_levels(model: Model, knobs: [f32; 3]) -> (Vec<f32>, Vec<f32>) {
    let mut s = board([Some(model), None, None]);
    s.knobs[model.index()] = knobs;
    s.mono = true;
    let mut f = fresh(&s);
    let n = (SR * 3.0) as usize;
    let w = std::f32::consts::TAU * 196.0 / SR;
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            0.3 * crate::math::exp(-t * 2.3) * crate::math::sin(w * i as f32)
        })
        .collect();
    let (y, _) = run(&mut f, &x, &x, 256);
    let win = (SR * 0.1) as usize;
    let lv = |v: &[f32]| {
        v.chunks(win)
            .map(|c| 10.0 * crate::math::log10(c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32 + 1e-30))
            .collect::<Vec<f32>>()
    };
    (lv(&x), lv(&y))
}

/// The Fuzz Face's Bias: with a fresh battery a decaying note sustains (the fuzz holds it
/// up as the input falls 24 dB); with a dying one it gates — the output collapses by more
/// than 30 dB while the input falls 12 dB, the sputtering end of a note on a dead battery.
#[test]
fn the_dying_battery_gates_a_decaying_note() {
    let (inp, fresh_out) = decay_levels(Model::Fuzz, [1.0, 1.0, 1.0]);
    let (_, dying) = decay_levels(Model::Fuzz, [1.0, 0.1, 1.0]);
    for (k, (i, (a, b))) in inp.iter().zip(fresh_out.iter().zip(&dying)).enumerate() {
        println!("{:.1} s: in {i:+6.1} dB, fresh {a:+6.1} dB, dying {b:+6.1} dB", k as f32 * 0.1);
    }
    // Windows where the input is at −30 and −54 dB(ish): 1.0 s and 2.0 s in (the input
    // falls 10 dB a second).
    let (early, late) = (5, 17);
    let fresh_drop = fresh_out[early] - fresh_out[late];
    assert!(fresh_drop < 6.0, "fresh battery: {fresh_drop} dB");
    // The dying battery: somewhere a 12 dB fall of the input (four windows) drops the
    // output by more than 30 dB.
    let gate = (0..inp.len() - 4)
        .map(|k| (dying[k] - dying[k + 4]) - (inp[k] - inp[k + 4]))
        .fold(f32::MIN, f32::max);
    assert!(gate > 18.0, "dying battery: the output falls only {gate} dB more than the input");
}

/// Aliasing: a 2.5 kHz sine at 300 mV through the op-amp distortion near full gain, at
/// 48 kHz: the energy that is not a harmonic of the note (what folded back), against the
/// harmonics', for each oversampling factor.
#[test]
fn aliasing_is_measured() {
    let n = 16_384;
    let k0 = 853; // 853 · 48 000 / 16 384 ≈ 2 499 Hz, on a bin
    let mut res = vec![];
    for os in FACTORS {
        let mut s = board([Some(Model::OpAmp), None, None]);
        s.knobs[Model::OpAmp.index()] = [0.8, 0.0, 1.0];
        s.oversampling = os;
        s.mono = true;
        let mut f = fresh(&s);
        let w = std::f64::consts::TAU * k0 as f64 / n as f64;
        let x: Vec<f32> = (0..3 * n)
            .map(|i| (0.3 * crate::math::sin(w * i as f64)) as f32)
            .collect();
        let (y, _) = run(&mut f, &x, &x, 512);
        let y = &y[2 * n..];
        let (mut harm, mut other) = (0.0f64, 0.0f64);
        for bin in 1..n / 2 {
            let wb = std::f64::consts::TAU * bin as f64 / n as f64;
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, v) in y.iter().enumerate() {
                let (si, co) = crate::math::sin_cos(wb * i as f64);
                re += *v as f64 * co;
                im += *v as f64 * si;
            }
            let p = re * re + im * im;
            if bin % k0 == 0 {
                harm += p;
            } else {
                other += p;
            }
        }
        let db = 10.0 * crate::math::log10(other / harm);
        println!("{os}x: aliasing {db:.1} dB below the harmonics");
        res.push(db);
    }
    assert!(res[0] > res[1] && res[1] > res[2], "{res:?}");
    assert!(res[1] < -50.0, "4x: {res:?}");
}

/// Each pedal at its defaults with its level at half: what comes out for a guitar-level
/// note (a pluck at −18 dBFS RMS), in dB against what went in. `CAL_DB` makes it about 0.
#[test]
fn calibration() {
    let x = pluck(147.0, 1.0, 1.0, 21);
    let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
    let g = 10f32.powf(-18.0 / 20.0) / rms(&x);
    let x: Vec<f32> = x.iter().map(|v| v * g).collect();
    let mut worst = vec![];
    for m in Model::ALL {
        let mut f = fresh(&board([Some(m), None, None]));
        let (y, _) = run(&mut f, &x, &x, 256);
        let d = 20.0 * crate::math::log10(rms(&y) / rms(&x));
        println!("{m:?}: {d:+.1} dB (CAL_DB {:+.1})", circuits::CAL_DB[m.index()]);
        worst.push((m, d));
    }
    assert!(worst.iter().all(|(_, d)| d.abs() < 1.0), "{worst:?}");
}

/// CPU per pedal: one second of a plucked note at 48 kHz, mono, each factor. Prints; run
/// it alone on a quiet machine (`--ignored --nocapture`).
#[test]
#[ignore]
fn cpu_per_pedal() {
    let x = pluck(110.0, 1.0, 0.4, 4);
    for os in FACTORS {
        for m in Model::ALL {
            let mut s = board([Some(m), None, None]);
            s.oversampling = os;
            s.mono = true;
            let mut f = fresh(&s);
            let mut best = f64::MAX;
            for _ in 0..3 {
                let (mut l, mut r) = (x.clone(), x.clone());
                let t = std::time::Instant::now();
                for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
                    f.process(a, b);
                }
                best = best.min(t.elapsed().as_secs_f64());
            }
            println!("{os}x {m:?}: {:.2} % of a core (mono, 48 kHz)", best * 100.0);
        }
    }
}
