//! Latitudo's tests: the null contract first, then each stage against its claim.

use super::view::{mono_pass, telemetry, unpack_point};
use super::*;
use std::f32::consts::{SQRT_2, TAU};

const SR: f32 = 48_000.0;

fn sine(hz: f32, amp: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| amp * (TAU * hz * i as f32 / SR).sin())
        .collect()
}

/// Deterministic noise, −1 … 1.
fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.max(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s as f32 / u32::MAX as f32 * 2.0 - 1.0
        })
        .collect()
}

/// A stereo programme: a centred tone, a left-leaning tone, noise in both (partly
/// correlated), bass out of phase — something for every stage.
fn programme(n: usize) -> (Vec<f32>, Vec<f32>) {
    let a = noise(n, 3);
    let b = noise(n, 11);
    let c = sine(440.0, 0.2, n);
    let d = sine(1_250.0, 0.15, n);
    let bass = sine(55.0, 0.2, n);
    let l = (0..n)
        .map(|i| 0.2 * a[i] + 0.05 * b[i] + c[i] + d[i] + bass[i])
        .collect();
    let r = (0..n)
        .map(|i| 0.2 * a[i] - 0.1 * b[i] + c[i] + 0.3 * d[i] - bass[i])
        .collect();
    (l, r)
}

fn run(s: &StereoSettings, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>, Latitudo) {
    let mut p = fresh(s);
    let (mut a, mut b) = (l.to_vec(), r.to_vec());
    let mut at = 0;
    while at < a.len() {
        let end = (at + block).min(a.len());
        p.process(&mut a[at..end], &mut b[at..end]);
        at = end;
    }
    (a, b, p)
}

/// An instance made the way the engine makes one: configured, then reset, so it starts
/// settled on its settings.
fn fresh(s: &StereoSettings) -> Latitudo {
    let mut p = Latitudo::new(SR);
    p.set(s);
    p.reset();
    p
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len().max(1) as f64).sqrt() as f32
}

fn db(x: f32) -> f32 {
    20.0 * x.max(1e-12).log10()
}

fn bits(x: &[f32]) -> Vec<u32> {
    x.iter().map(|v| v.to_bits()).collect()
}

/// Everything at once: every stage working.
fn busy() -> StereoSettings {
    StereoSettings {
        width: 1.3,
        mid_db: -1.0,
        side_db: 2.0,
        balance: 0.1,
        rotation_deg: 5.0,
        invert_r: true,
        mono_hz: 120.0,
        mono_slope: Slope::S48,
        w_lo: 0.5,
        w_hi: 1.6,
        mid_lo_db: 2.0,
        side_hi_db: 3.0,
        tone_slope: Slope::S12,
        haas_ms: 8.0,
        haas_mode: HaasMode::MonoSafe,
        haas_amount: 0.4,
        out_db: -2.0,
        ..StereoSettings::default()
    }
}

// ---- the null contract ---------------------------------------------------------------

/// At its defaults the effect returns its input to the bit — signed zeros, the smallest
/// numbers and full scale included — and says it is a wire.
#[test]
fn at_its_defaults_the_output_is_the_input_to_the_bit() {
    let (mut l, mut r) = programme(48_000);
    l[10] = -0.0;
    r[11] = -0.0;
    l[12] = f32::MIN_POSITIVE;
    r[13] = 1.0;
    l[14] = -1.0;
    for block in [1, 64, 512, 4096] {
        let (a, b, p) = run(&StereoSettings::default(), &l, &r, block);
        assert_eq!(bits(&a), bits(&l), "left, block {block}");
        assert_eq!(bits(&b), bits(&r), "right, block {block}");
        assert!(p.is_wire());
        assert_eq!(p.latency(), 0);
    }
}

/// Every stage at a neutral value is a wire too: shelves at 0 dB wherever their
/// frequencies are, every band at 100 % wherever the crossovers are, the Haas amount
/// with no delay, other slopes and modes.
#[test]
fn neutral_values_of_every_stage_are_wires() {
    let (l, r) = programme(24_000);
    let s = StereoSettings {
        x_lo: 90.0,
        x_hi: 9_000.0,
        mid_lo_hz: 400.0,
        side_hi_hz: 3_000.0,
        tone_slope: Slope::S12,
        mono_slope: Slope::S6,
        haas_mode: HaasMode::MonoSafe,
        haas_amount: 1.0,
        ..StereoSettings::default()
    };
    let (a, b, p) = run(&s, &l, &r, 333);
    assert_eq!(bits(&a), bits(&l));
    assert_eq!(bits(&b), bits(&r));
    assert!(p.is_wire());
}

/// Every control moved and then put back: once the glides and fades have landed, the
/// effect is a wire again, to the bit.
#[test]
fn every_control_put_back_is_a_wire_again() {
    let (l, r) = programme(96_000);
    let mut p = Latitudo::new(SR);
    p.set(&busy());
    let (mut a, mut b) = (l[..48_000].to_vec(), r[..48_000].to_vec());
    p.process(&mut a, &mut b);
    assert!(!p.is_wire());
    p.set(&StereoSettings::default());
    let (mut a, mut b) = (l[48_000..].to_vec(), r[48_000..].to_vec());
    p.process(&mut a, &mut b);
    assert!(
        p.is_wire(),
        "still working after the reset of every control: full {} t {:?} fades {:?} {:?} {:?} {:?} live {:?} settled {}",
        p.full,
        p.t,
        p.mono_fade,
        p.band_fade,
        p.tone_fade,
        p.haas_fade,
        p.shelf_live,
        p.settled.is_some()
    );
    // The last half second is the input itself.
    let tail = 24_000;
    assert_eq!(bits(&a[tail..]), bits(&l[48_000 + tail..]));
    assert_eq!(bits(&b[tail..]), bits(&r[48_000 + tail..]));
}

/// The output does not depend on how the stream is cut into blocks: every stage on, at
/// tolerance zero, meters included.
#[test]
fn the_block_size_does_not_matter_to_the_bit() {
    let (l, r) = programme(30_000);
    let s = busy();
    let (ra, rb, rp) = run(&s, &l, &r, 512);
    let mut rt = [0.0; 64];
    rp.write_telemetry(&mut rt);
    assert!(ra.iter().chain(&rb).all(|x| x.is_finite()));
    for block in [1, 7, 16, 64, 1000, 4096] {
        let (a, b, p) = run(&s, &l, &r, block);
        assert_eq!(bits(&a), bits(&ra), "left, block {block}");
        assert_eq!(bits(&b), bits(&rb), "right, block {block}");
        let mut t = [0.0; 64];
        p.write_telemetry(&mut t);
        // Everything but the peaks, which are the last block's by definition.
        for k in (0..64).filter(|k| ![telemetry::PEAK_L, telemetry::PEAK_R].contains(k)) {
            assert_eq!(
                t[k].to_bits(),
                rt[k].to_bits(),
                "telemetry {k}, block {block}"
            );
        }
    }
}

/// Two instances given the same input give the same output; a reset instance gives it
/// again.
#[test]
fn it_is_deterministic_and_reset_starts_over() {
    let (l, r) = programme(20_000);
    let (a1, b1, mut p) = run(&busy(), &l, &r, 256);
    let (a2, b2, _) = run(&busy(), &l, &r, 256);
    assert_eq!(bits(&a1), bits(&a2));
    assert_eq!(bits(&b1), bits(&b2));
    p.reset();
    let (mut a3, mut b3) = (l.clone(), r.clone());
    p.process(&mut a3, &mut b3);
    assert_eq!(bits(&a3), bits(&a1), "a reset instance");
    assert_eq!(bits(&b3), bits(&b1));
}

/// Silence stays exactly silent with everything on, and a programme followed by silence
/// rings down to exact zeros: no denormal is left in any state.
#[test]
fn silence_is_silent_and_the_state_flushes_to_zero() {
    let z = vec![0.0f32; 48_000];
    let (a, b, _) = run(&busy(), &z, &z, 512);
    assert!(a.iter().chain(&b).all(|x| *x == 0.0));
    let (mut l, mut r) = programme(24_000);
    l.extend(std::iter::repeat_n(0.0, 5 * 48_000));
    r.extend(std::iter::repeat_n(0.0, 5 * 48_000));
    let (a, b, _) = run(&busy(), &l, &r, 512);
    let last = &a[a.len() - 4_800..];
    assert!(last.iter().all(|x| *x == 0.0), "the left rings on");
    assert!(
        b[b.len() - 4_800..].iter().all(|x| *x == 0.0),
        "the right rings on"
    );
    assert!(a
        .iter()
        .chain(&b)
        .all(|x| x.is_finite() && !x.is_subnormal()));
}

/// A full-scale square through every stage at its extremes: finite, and bounded by what
/// the gains allow (width 200 %, side +12 dB, shelves +12 dB, output +12 dB).
#[test]
fn a_full_scale_square_at_the_extremes_stays_finite_and_bounded() {
    let n = 48_000;
    let sq: Vec<f32> = (0..n)
        .map(|i| if (i / 60) % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let neg: Vec<f32> = sq.iter().map(|x| -x * 0.5).collect();
    let s = StereoSettings {
        width: 2.0,
        side_db: 12.0,
        mid_db: 12.0,
        w_lo: 2.0,
        w_mid: 2.0,
        w_hi: 2.0,
        mid_lo_db: 12.0,
        mid_hi_db: 12.0,
        side_lo_db: 12.0,
        side_hi_db: 12.0,
        mono_hz: 1_000.0,
        haas_ms: 30.0,
        haas_mode: HaasMode::MonoSafe,
        haas_amount: 1.0,
        out_db: 12.0,
        rotation_deg: 45.0,
        ..StereoSettings::default()
    };
    let (a, b, _) = run(&s, &sq, &neg, 512);
    let peak = a.iter().chain(&b).fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak.is_finite());
    // 4 (mid) · 4 · 4 (two shelves) · 4 (out) · √2 (rotation), and the mono-safe side —
    // a loose ceiling that only a blow-up would break.
    assert!(peak < 1_000.0, "peak {peak}");
}

// ---- the image -------------------------------------------------------------------------

/// After a settings change, the samples once every glide has landed.
fn settled(s: &StereoSettings, l: &[f32], r: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let (a, b, _) = run(s, l, r, 512);
    let from = 9_600; // 200 ms
    (a[from..].to_vec(), b[from..].to_vec())
}

#[test]
fn width_zero_is_mono_and_two_doubles_the_side() {
    let (l, r) = programme(24_000);
    let from = 9_600;
    let (a, b) = settled(
        &StereoSettings {
            width: 0.0,
            ..Default::default()
        },
        &l,
        &r,
    );
    for i in 0..a.len() {
        let m = 0.5 * (l[from + i] + r[from + i]);
        assert!((a[i] - m).abs() < 1e-6 && (b[i] - m).abs() < 1e-6);
    }
    let (a, b) = settled(
        &StereoSettings {
            width: 2.0,
            ..Default::default()
        },
        &l,
        &r,
    );
    for i in 0..a.len() {
        let (x, y) = (l[from + i], r[from + i]);
        let (m, s) = (0.5 * (x + y), 0.5 * (x - y));
        assert!((a[i] - (m + 2.0 * s)).abs() < 1e-5);
        assert!((b[i] - (m - 2.0 * s)).abs() < 1e-5);
    }
}

#[test]
fn swap_polarity_balance_and_listen_do_what_they_say() {
    let (l, r) = programme(24_000);
    let from = 9_600;
    let tail = |v: &[f32]| v[from..].to_vec();
    // Swap: exactly the other channel.
    let (a, b) = settled(
        &StereoSettings {
            swap: true,
            ..Default::default()
        },
        &l,
        &r,
    );
    assert_eq!(bits(&a), bits(&tail(&r)));
    assert_eq!(bits(&b), bits(&tail(&l)));
    // Polarity of the left alone.
    let (a, b) = settled(
        &StereoSettings {
            invert_l: true,
            ..Default::default()
        },
        &l,
        &r,
    );
    let neg: Vec<f32> = tail(&l).iter().map(|x| -x).collect();
    assert_eq!(bits(&a), bits(&neg));
    assert_eq!(bits(&b), bits(&tail(&r)));
    // Balance hard right: the left is gone, the right untouched.
    let (a, b) = settled(
        &StereoSettings {
            balance: 1.0,
            ..Default::default()
        },
        &l,
        &r,
    );
    assert!(a.iter().all(|x| x.abs() < 1e-6));
    for (x, y) in b.iter().zip(tail(&r)) {
        assert!((x - y).abs() < 1e-6);
    }
    // Listen to the mid, then to the side: the same in both speakers.
    for (listen, sign) in [(Listen::Mid, 1.0), (Listen::Side, -1.0)] {
        let (a, b) = settled(
            &StereoSettings {
                listen,
                ..Default::default()
            },
            &l,
            &r,
        );
        for i in 0..a.len() {
            let want = 0.5 * (l[from + i] + sign * r[from + i]);
            assert!(
                (a[i] - want).abs() < 1e-6 && (b[i] - want).abs() < 1e-6,
                "{listen:?}"
            );
        }
    }
}

/// Rotation turns the image: a centred source at +45° comes out of the right speaker
/// alone, with its power kept.
#[test]
fn rotation_turns_a_centred_source_to_a_speaker() {
    let x = sine(500.0, 0.4, 24_000);
    let (a, b) = settled(
        &StereoSettings {
            rotation_deg: 45.0,
            ..Default::default()
        },
        &x,
        &x,
    );
    assert!(rms(&a) < 1e-6, "left {}", rms(&a));
    assert!((rms(&b) - SQRT_2 * rms(&x[9_600..])).abs() < 1e-4);
    let (a, b) = settled(
        &StereoSettings {
            rotation_deg: -45.0,
            ..Default::default()
        },
        &x,
        &x,
    );
    assert!(rms(&b) < 1e-6);
    assert!((rms(&a) - SQRT_2 * rms(&x[9_600..])).abs() < 1e-4);
}

/// The gains of the mid and of the side, and the output.
#[test]
fn mid_side_and_output_gains() {
    let (l, r) = programme(24_000);
    let from = 9_600;
    let s = StereoSettings {
        mid_db: -6.0,
        side_db: 6.0,
        out_db: -3.0,
        ..Default::default()
    };
    let (a, b) = settled(&s, &l, &r);
    let (gm, gs, go) = (db_to_gain(-6.0), db_to_gain(6.0), db_to_gain(-3.0));
    for i in 0..a.len() {
        let (x, y) = (l[from + i], r[from + i]);
        let (m, sd) = (0.5 * (x + y) * gm, 0.5 * (x - y) * gs);
        assert!((a[i] - go * (m + sd)).abs() < 1e-5);
        assert!((b[i] - go * (m - sd)).abs() < 1e-5);
    }
}

// ---- mono below, width by band, tone ----------------------------------------------------

/// The level of the side and of the mid of a settled output, dB.
fn ms_levels(a: &[f32], b: &[f32]) -> (f32, f32) {
    let m: Vec<f32> = a.iter().zip(b).map(|(x, y)| 0.5 * (x + y)).collect();
    let s: Vec<f32> = a.iter().zip(b).map(|(x, y)| 0.5 * (x - y)).collect();
    (db(rms(&m)), db(rms(&s)))
}

/// A side-only tone (left = x, right = -x) and a mid-only tone (left = right = x).
fn side_tone(hz: f32) -> (Vec<f32>, Vec<f32>) {
    let x = sine(hz, 0.25, 48_000);
    let y = x.iter().map(|v| -v).collect();
    (x, y)
}

fn mid_tone(hz: f32) -> (Vec<f32>, Vec<f32>) {
    let x = sine(hz, 0.25, 48_000);
    (x.clone(), x)
}

/// Mono below 150 Hz: a side tone an octave and a half under it is gone, one well above
/// it passes, and the mid is untouched in level at any frequency.
#[test]
fn mono_below_removes_the_low_side_and_keeps_the_rest() {
    let level = db(0.25 / SQRT_2);
    for slope in MONO_SLOPES {
        let s = StereoSettings {
            mono_hz: 150.0,
            mono_slope: slope,
            ..Default::default()
        };
        let (l, r) = side_tone(50.0);
        let (a, b) = settled(&s, &l, &r);
        let (_, side) = ms_levels(&a, &b);
        let want = 20.0 * mono_pass(&s, SR, 50.0).log10() + level;
        assert!(
            (side - want).abs() < 0.5,
            "{slope:?}: side {side} want {want}"
        );
        let need = match slope {
            Slope::S6 => 9.0,
            Slope::S12 => 18.0,
            Slope::S24 => 36.0,
            Slope::S48 => 60.0,
        };
        assert!(level - side > need, "{slope:?} took {} dB", level - side);
        let (l, r) = side_tone(3_000.0);
        let (a, b) = settled(&s, &l, &r);
        assert!(
            (ms_levels(&a, &b).1 - level).abs() < 0.05,
            "{slope:?} high side"
        );
        for hz in [40.0, 150.0, 2_000.0] {
            let (l, r) = mid_tone(hz);
            let (a, b) = settled(&s, &l, &r);
            let (mid, side) = ms_levels(&a, &b);
            assert!((mid - level).abs() < 0.01, "{slope:?} mid at {hz}: {mid}");
            assert!(side < -100.0, "{slope:?}: a mid tone grew a side");
        }
    }
}

/// Why the mono crossover gives the mid its all-pass: a hard-left source above the
/// crossover stays hard left. (A plain high-pass on the side alone would leak it into the
/// right speaker around and above the cut-off.)
#[test]
fn mono_below_keeps_the_image_above_it() {
    let x = sine(600.0, 0.3, 48_000);
    let z = vec![0.0; 48_000];
    // The leak is the crossover's low-pass at that frequency, halved: what a plain
    // high-pass on the side would leak is many times more (about −17 dB for 12 dB/oct).
    for (slope, most) in [
        (Slope::S12, -30.0),
        (Slope::S24, -50.0),
        (Slope::S48, -50.0),
    ] {
        let s = StereoSettings {
            mono_hz: 120.0,
            mono_slope: slope,
            ..Default::default()
        };
        let (a, b) = settled(&s, &x, &z);
        let leak = db(rms(&b)) - db(rms(&a));
        assert!(
            leak < most,
            "{slope:?}: the right gets {leak} dB of a left source"
        );
    }
}

/// Width by band: the low band narrowed, the high band widened; the mid untouched; the
/// levels as the response the editor draws says.
#[test]
fn width_by_band_follows_its_response() {
    let s = StereoSettings {
        w_lo: 0.0,
        w_mid: 1.0,
        w_hi: 2.0,
        ..Default::default()
    };
    let level = db(0.25 / SQRT_2);
    for hz in [40.0, 250.0, 1_000.0, 4_000.0, 12_000.0] {
        let (l, r) = side_tone(hz);
        let (a, b) = settled(&s, &l, &r);
        let side = ms_levels(&a, &b).1;
        let want = level + db(band_width(&s, SR, hz));
        assert!(
            (side - want).abs() < 0.2,
            "side at {hz}: {side} want {want}"
        );
        let (l, r) = mid_tone(hz);
        let (a, b) = settled(&s, &l, &r);
        assert!((ms_levels(&a, &b).0 - level).abs() < 0.01, "mid at {hz}");
    }
    assert!(band_width(&s, SR, 30.0) < 0.05);
    assert!((band_width(&s, SR, 15_000.0) - 2.0).abs() < 0.05);
    assert!((width_at(&s, SR, 15_000.0) - 2.0).abs() < 0.05);
}

/// The tone section: shelves on the mid and on the side, each heard where the response
/// the editor draws says, and only on its own channel.
#[test]
fn the_tone_shelves_act_on_their_own_channel_as_drawn() {
    for slope in TONE_SLOPES {
        let s = StereoSettings {
            mid_hi_db: 6.0,
            mid_hi_hz: 6_000.0,
            side_lo_db: -6.0,
            side_lo_hz: 300.0,
            tone_slope: slope,
            ..Default::default()
        };
        let level = db(0.25 / SQRT_2);
        for hz in [60.0, 300.0, 1_000.0, 6_000.0, 15_000.0] {
            let (want_m, want_s) = tone_db(&s, SR, hz);
            let (l, r) = mid_tone(hz);
            let (a, b) = settled(&s, &l, &r);
            let (mid, side) = ms_levels(&a, &b);
            assert!(
                (mid - level - want_m).abs() < 0.1,
                "{slope:?} mid {hz}: {mid}"
            );
            assert!(side < -100.0);
            let (l, r) = side_tone(hz);
            let (a, b) = settled(&s, &l, &r);
            let (mid, side) = ms_levels(&a, &b);
            assert!(
                (side - level - want_s).abs() < 0.1,
                "{slope:?} side {hz}: {side}"
            );
            assert!(mid < -100.0);
        }
        let (m, _) = tone_db(&s, SR, 18_000.0);
        assert!(m > 5.0, "{slope:?}: {m}");
        let (_, sd) = tone_db(&s, SR, 40.0);
        assert!(sd < -5.0, "{slope:?}: {sd}");
    }
}

// ---- Haas ------------------------------------------------------------------------------

/// Classic Haas: the right channel, once the fade is done, is the input exactly the delay
/// late; the left is untouched; the editor's comb frequency is where mono cancels.
#[test]
fn classic_haas_delays_one_channel() {
    let (l, r) = programme(48_000);
    let s = StereoSettings {
        haas_ms: 10.0,
        ..Default::default()
    };
    let (a, b) = settled(&s, &l, &r);
    let d = 480;
    let from = 9_600;
    for i in 0..a.len() {
        assert!((a[i] - l[from + i]).abs() < 1e-6);
        assert!((b[i] - r[from + i - d]).abs() < 1e-6, "right at {i}");
    }
    assert_eq!(s.mono_comb_hz(), Some(50.0));
    // At that frequency a centred tone cancels in mono.
    let x = sine(50.0, 0.3, 48_000);
    let (a, b) = settled(&s, &x, &x);
    let mono: Vec<f32> = a.iter().zip(&b).map(|(p, q)| p + q).collect();
    assert!(db(rms(&mono)) < db(rms(&x)) - 40.0);
}

/// Mono-safe Haas widens (the side grows from a centred source) and the mono sum is the
/// input's, untouched.
#[test]
fn mono_safe_haas_leaves_the_mono_sum_alone() {
    let x = sine(700.0, 0.3, 48_000);
    let s = StereoSettings {
        haas_ms: 12.0,
        haas_mode: HaasMode::MonoSafe,
        haas_amount: 0.8,
        ..Default::default()
    };
    let (a, b, _) = run(&s, &x, &x, 512);
    for i in 0..a.len() {
        assert!((a[i] + b[i] - 2.0 * x[i]).abs() < 1e-5, "mono sum at {i}");
    }
    let side: Vec<f32> = a.iter().zip(&b).map(|(p, q)| p - q).collect();
    assert!(rms(&side[9_600..]) > 0.1, "no width was made");
    assert_eq!(s.mono_comb_hz(), None);
}

// ---- the meters ------------------------------------------------------------------------

fn meters(l: &[f32], r: &[f32], s: &StereoSettings) -> [f32; 64] {
    let (_, _, p) = run(s, l, r, 512);
    let mut t = [0.0; 64];
    p.write_telemetry(&mut t);
    t
}

/// Correlation: +1 for mono, −1 for one side inverted, near 0 for unrelated noise;
/// balance −1 for the left alone; the side against the mid at the floor for mono.
#[test]
fn correlation_and_balance_read_what_they_should() {
    use telemetry::*;
    let x = sine(300.0, 0.3, 24_000);
    let neg: Vec<f32> = x.iter().map(|v| -v).collect();
    let z = vec![0.0; 24_000];
    let d = StereoSettings::default();
    let t = meters(&x, &x, &d);
    assert!((t[CORR] - 1.0).abs() < 1e-3 && t[BAL].abs() < 1e-3);
    assert_eq!(t[SIDE_DB], view::SIDE_FLOOR_DB);
    let t = meters(&x, &neg, &d);
    assert!((t[CORR] + 1.0).abs() < 1e-3);
    let t = meters(&noise(24_000, 5), &noise(24_000, 99), &d);
    assert!(t[CORR].abs() < 0.1, "noise {}", t[CORR]);
    assert!(
        t[SIDE_DB].abs() < 1.0,
        "unrelated channels: side and mid equal"
    );
    let t = meters(&x, &z, &d);
    assert!((t[BAL] + 1.0).abs() < 1e-3 && t[CORR] == 0.0);
    // The input's meters see what came in, the output's what went out.
    let t = meters(&x, &neg, &StereoSettings { width: 0.0, ..d });
    assert!((t[IN_CORR] + 1.0).abs() < 1e-3);
    assert!(
        t[LEVEL_DB] < -100.0,
        "an out-of-phase pair made mono is silence"
    );
    assert!(t[IN_LEVEL_DB] > -15.0);
}

/// The correlation per band tells out-of-phase bass from a centred top.
#[test]
fn band_correlation_finds_the_out_of_phase_bass() {
    use telemetry::*;
    let n = 48_000;
    // A little centred bass, more of it out of phase, and a centred top.
    let bass = sine(45.0, 0.1, n);
    let wide = sine(60.0, 0.3, n);
    let top = sine(9_000.0, 0.2, n);
    let l: Vec<f32> = (0..n).map(|i| bass[i] + wide[i] + top[i]).collect();
    let r: Vec<f32> = (0..n).map(|i| bass[i] - wide[i] + top[i]).collect();
    let t = meters(&l, &r, &StereoSettings::default());
    assert!(t[BAND_CORR] < -0.6, "low band {}", t[BAND_CORR]);
    assert!(t[BAND_CORR + 2] > 0.9, "high band {}", t[BAND_CORR + 2]);
    // Mono below 150 Hz, steep, mends it.
    let s = StereoSettings {
        mono_hz: 150.0,
        mono_slope: Slope::S48,
        ..Default::default()
    };
    let t = meters(&l, &r, &s);
    assert!(t[BAND_CORR] > 0.9, "mended: {}", t[BAND_CORR]);
}

/// The vectorscope's points are the output's samples, packed within a 2048th of the
/// range, counted, oldest first.
#[test]
fn the_points_are_the_output() {
    use telemetry::*;
    let n = 4_800;
    let l = sine(100.0, 0.5, n);
    let r = sine(100.0, 0.25, n);
    let mut p = Latitudo::new(SR);
    let (mut a, mut b) = (l.clone(), r.clone());
    p.process(&mut a, &mut b);
    let mut t = [0.0; 64];
    p.write_telemetry(&mut t);
    let stride = t[STRIDE] as usize;
    assert_eq!(stride, (SR / POINT_RATE).round() as usize);
    let written = n.div_ceil(stride);
    assert_eq!(t[SEQ] as usize, written);
    let newest = (written - 1) * stride;
    let (x, y) = unpack_point(t[POINTS_AT + POINTS - 1]);
    assert!((x - l[newest]).abs() < 1e-3 && (y - r[newest]).abs() < 1e-3);
    for (v, w) in [(0.0, 0.0), (1.0, -1.0), (-2.5, 7.0), (0.3, f32::NAN)] {
        let (x, y) = unpack_point(view::pack_point(v, w));
        assert!(x.is_finite() && y.is_finite());
        assert!((x - v.clamp(-2.0, 2.0)).abs() < 1e-3 || !w.is_finite());
    }
    assert_eq!(telemetry::since(3.0, (WRAP - 2) as f32), 5);
}

// ---- glides and fades: nothing clicks --------------------------------------------------

/// The largest step between two neighbouring samples.
fn max_step(x: &[f32]) -> f32 {
    x.windows(2).fold(0.0, |m, w| m.max((w[1] - w[0]).abs()))
}

/// Every switch thrown while a low tone plays: polarity, swap, listen, width to nothing,
/// a slope change while the mono crossover works, a Haas mode change. The tone's own
/// steepest step is tiny; no switch may make a step much bigger than a few of those.
#[test]
fn switches_glide_and_fade_without_a_click() {
    let n = 48_000;
    let l = sine(80.0, 0.5, n);
    let r: Vec<f32> = sine(80.0, 0.5, n).iter().map(|v| v * 0.3).collect();
    let own = max_step(&l);
    let base = StereoSettings {
        mono_hz: 200.0,
        haas_ms: 5.0,
        ..Default::default()
    };
    let changes: [fn(&mut StereoSettings); 8] = [
        |s| s.invert_l = true,
        |s| s.swap = true,
        |s| s.listen = Listen::Side,
        |s| s.width = 0.0,
        |s| s.mono_slope = Slope::S48,
        |s| s.mono_hz = MONO_OFF_HZ,
        |s| s.haas_mode = HaasMode::DelayLeft,
        |s| s.tone_slope = Slope::S12,
    ];
    for (k, change) in changes.iter().enumerate() {
        let mut s = base;
        s.mid_hi_db = 3.0;
        let mut p = Latitudo::new(SR);
        p.set(&s);
        let (mut a, mut b) = (l.clone(), r.clone());
        let half = n / 2 + 5;
        p.process(&mut a[..half], &mut b[..half]);
        change(&mut s);
        p.set(&s);
        p.process(&mut a[half..], &mut b[half..]);
        let around = half - 200..half + 2_000;
        let step = max_step(&a[around.clone()]).max(max_step(&b[around]));
        assert!(
            step < 4.0 * own,
            "change {k}: a step of {step} (the tone's own: {own})"
        );
    }
}
