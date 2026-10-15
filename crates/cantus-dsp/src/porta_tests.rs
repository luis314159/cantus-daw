//! Porta's tests: the null contract first, then the behaviour, each with its number.

use super::*;
use crate::math;
use std::f32::consts::TAU;

const SR: f32 = 48_000.0;

fn ms(x: f32) -> usize {
    (x * 0.001 * SR).round() as usize
}

/// A sine of `amp` peak at `hz`, starting at `phase` radians.
fn sine(hz: f32, amp: f32, n: usize, phase: f32) -> Vec<f32> {
    (0..n)
        .map(|i| amp * math::sin(TAU * hz * i as f32 / SR + phase))
        .collect()
}

fn db(x: f32) -> f32 {
    math::powf(10f32, x / 20.0)
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

fn gate(s: GateSettings) -> Porta {
    let mut p = Porta::new(SR);
    p.set(&s);
    p
}

/// Run a whole signal through at a block size; `key` is the external key, if any.
fn run(
    s: &GateSettings,
    l: &[f32],
    r: &[f32],
    key: Option<(&[f32], &[f32])>,
    block: usize,
) -> (Vec<f32>, Vec<f32>, [f32; 64]) {
    let mut p = gate(*s);
    let (mut a, mut b) = (l.to_vec(), r.to_vec());
    let mut at = 0;
    while at < a.len() {
        let end = (at + block).min(a.len());
        let k = key.map(|(x, y)| (&x[at..end], &y[at..end]));
        p.process(&mut a[at..end], &mut b[at..end], k);
        at = end;
    }
    let mut t = [0.0; 64];
    p.write_telemetry(&mut t);
    (a, b, t)
}

/// Sample by sample: the phase, gain and detector level after each one.
fn probe(s: &GateSettings, x: &[f32], key: Option<&[f32]>) -> Vec<(Phase, f32, f32)> {
    let mut p = gate(*s);
    let mut out = Vec::with_capacity(x.len());
    for i in 0..x.len() {
        let (mut a, mut b) = ([x[i]], [x[i]]);
        let k = key.map(|k| [k[i]]);
        p.process(&mut a, &mut b, k.as_ref().map(|k| (&k[..], &k[..])));
        out.push((p.phase(), p.gain_db(), p.level_db()));
    }
    out
}

fn settings(style: Style) -> GateSettings {
    GateSettings {
        style,
        ..GateSettings::default()
    }
}

// ---- the null contract ---------------------------------------------------------------

/// A signal that never falls below the threshold comes out as it went in, to the bit — in
/// every style that lowers what is below (Upward: a signal that never rises above it).
#[test]
fn a_signal_that_never_crosses_the_threshold_comes_out_to_the_bit() {
    let n = 48_000;
    // Two partials, starting at a peak (a cosine), −6 dBFS: far above −40 dB.
    let loud: Vec<f32> = sine(220.0, 0.35, n, TAU / 4.0)
        .iter()
        .zip(sine(1_330.0, 0.15, n, TAU / 4.0))
        .map(|(a, b)| a + b)
        .collect();
    let right: Vec<f32> = loud.iter().map(|x| x * 0.7).collect();
    for style in [Style::Classic, Style::Clean, Style::Vocal, Style::Guitar] {
        let s = GateSettings {
            hpf_hz: 60.0,
            lpf_hz: 12_000.0,
            ..settings(style)
        };
        let (l, r, _) = run(&s, &loud, &right, None, 512);
        assert_eq!(l, loud, "{style:?} touched a signal above its threshold");
        assert_eq!(r, right, "{style:?}, right channel");
    }
    // Upward: a quiet signal, all of it below the threshold.
    let quiet: Vec<f32> = loud.iter().map(|x| x * 0.01).collect(); // about −46 dBFS
    let s = GateSettings {
        threshold_db: -20.0,
        ratio: 2.0,
        ..settings(Style::Upward)
    };
    let (l, _, _) = run(&s, &quiet, &quiet, None, 512);
    assert_eq!(l, quiet, "Upward touched a signal below its threshold");
    // Ducking: the key never reaches the threshold.
    let key = sine(100.0, 0.001, n, 0.0); // −60 dBFS
    let s = settings(Style::Ducking);
    let (l, _, _) = run(&s, &loud, &loud, Some((&key, &key)), 512);
    assert_eq!(l, loud, "Ducking moved with a quiet key");
}

/// With lookahead, the wire is the input delayed by exactly the latency.
#[test]
fn with_lookahead_the_wire_is_the_input_delayed_by_the_latency() {
    let n = 24_000;
    let x = sine(440.0, 0.5, n, TAU / 4.0);
    for look in [0.5, 3.0, 10.0] {
        let s = GateSettings {
            lookahead_ms: look,
            ..settings(Style::Clean)
        };
        let d = gate(s).latency();
        assert_eq!(
            d,
            ms(LOOKAHEAD_MAX_MS),
            "the latency does not follow the knob"
        );
        let (l, _, _) = run(&s, &x, &x, None, 333);
        assert!(l[..d].iter().all(|v| *v == 0.0));
        assert_eq!(&l[d..], &x[..n - d], "lookahead {look} ms");
    }
    let off = GateSettings {
        lookahead_ms: 0.0,
        ..GateSettings::default()
    };
    assert_eq!(gate(off).latency(), 0, "off costs nothing");
    let mut p44 = Porta::new(44_100.0);
    p44.set(&GateSettings {
        lookahead_ms: 2.0,
        ..GateSettings::default()
    });
    assert_eq!(p44.latency(), 441);
}

/// At range 0 nothing moves, whatever the signal does.
#[test]
fn range_zero_is_a_wire_whatever_the_signal() {
    let mut x = sine(150.0, 0.8, 30_000, 0.0);
    for v in &mut x[10_000..20_000] {
        *v *= 0.0001;
    }
    for style in Style::ALL {
        let s = GateSettings {
            range_db: 0.0,
            ..settings(style)
        };
        let (l, _, _) = run(&s, &x, &x, None, 256);
        assert_eq!(l, x, "{style:?} at range 0");
    }
}

/// Every style, with filters, lookahead, hold and knee, at block sizes 1 to 4096: the same
/// samples and the same telemetry.
#[test]
fn the_result_does_not_depend_on_the_block_size() {
    let n = 40_000;
    let mut x = sine(90.0, 0.6, n, 0.0);
    for (i, v) in x.iter_mut().enumerate() {
        // Hits every 150 ms, decaying, over a quiet floor.
        let t = (i % ms(150.0)) as f32 / SR;
        *v *= math::exp(-t * 30.0) + 0.003;
    }
    let right: Vec<f32> = x.iter().map(|v| -v * 0.5).collect();
    let key = sine(3_000.0, 0.2, n, 0.3);
    for style in Style::ALL {
        let s = GateSettings {
            threshold_db: -24.0,
            hysteresis_db: 3.0,
            ratio: 4.0,
            knee_db: 6.0,
            hold_ms: 12.0,
            attack_ms: 0.7,
            release_ms: 60.0,
            lookahead_ms: 2.5,
            hpf_hz: 70.0,
            lpf_hz: 8_000.0,
            key_slope: 48.0,
            ..settings(style)
        };
        let k = (style == Style::Ducking).then_some((&key[..], &key[..]));
        let reference = run(&s, &x, &right, k, n);
        for block in [1usize, 7, 64, 1000, 4096] {
            let other = run(&s, &x, &right, k, block);
            assert_eq!(other.0, reference.0, "{style:?} at block {block}");
            assert_eq!(other.1, reference.1, "{style:?} at block {block}, right");
            assert_eq!(other.2, reference.2, "{style:?} telemetry at block {block}");
        }
    }
}

/// Silence after a burst is exact silence, with every ringing state at rest: no denormal
/// can stall the CPU, and no NaN appears.
#[test]
fn silence_stays_silent_and_nothing_stalls() {
    for style in Style::ALL {
        let mut x = sine(60.0, 1.0, ms(200.0), 0.0);
        x.extend(std::iter::repeat_n(0.0, ms(10_000.0)));
        let s = GateSettings {
            hpf_hz: 40.0,
            lpf_hz: 5_000.0,
            lookahead_ms: 4.0,
            range_db: 100.0,
            ..settings(style)
        };
        let mut p = gate(s);
        let (mut l, mut r) = (x.clone(), x.clone());
        p.process(&mut l, &mut r, None);
        assert!(l.iter().all(|v| v.is_finite()), "{style:?}");
        assert!(
            l[l.len() - ms(1_000.0)..].iter().all(|v| *v == 0.0),
            "{style:?}"
        );
        assert!(p.is_quiet(), "{style:?} still rings after 10 s of silence");
    }
}

// ---- behaviour, with numbers ---------------------------------------------------------

/// The static curve is the one drawn: expansion by the ratio, capped by the range; a
/// gate at ∞; a boost above for Upward; a cut above for Ducking.
#[test]
fn the_static_curve_is_the_one_drawn() {
    let s = GateSettings {
        threshold_db: -30.0,
        hysteresis_db: 5.0,
        ratio: 2.0,
        range_db: 40.0,
        knee_db: 0.0,
        ..GateSettings::default()
    };
    assert_eq!(static_gain_db(-20.0, &s, false), 0.0, "above: nothing");
    assert!(
        (static_gain_db(-40.0, &s, false) + 10.0).abs() < 1e-4,
        "2:1: 10 below → −10"
    );
    assert!(
        (static_gain_db(-40.0, &s, true) + 5.0).abs() < 1e-4,
        "an open gate measures from the close threshold, 5 dB lower"
    );
    assert_eq!(
        static_gain_db(-100.0, &s, false),
        -40.0,
        "the range caps it"
    );
    let g = GateSettings { ratio: 100.0, ..s };
    assert_eq!(
        static_gain_db(-30.5, &g, false),
        -40.0,
        "∞: half a dB below is closed"
    );
    let soft = GateSettings { knee_db: 10.0, ..s };
    assert!(
        static_gain_db(-27.0, &soft, false) < 0.0,
        "the knee starts above"
    );
    assert!(
        (static_gain_db(-40.0, &soft, false) + 10.0).abs() < 1e-4,
        "below the knee, as hard"
    );
    let up = GateSettings {
        ratio: 2.0,
        range_db: 100.0,
        style: Style::Upward,
        ..s
    };
    assert!(
        (static_gain_db(-24.0, &up, false) - 6.0).abs() < 1e-4,
        "6 over at 2:1 → +6"
    );
    assert_eq!(
        static_gain_db(20.0, &up, false),
        MAX_BOOST_DB,
        "the boost is capped"
    );
    assert_eq!(static_gain_db(-50.0, &up, false), 0.0);
    let duck = GateSettings {
        ratio: 100.0,
        range_db: 12.0,
        style: Style::Ducking,
        ..s
    };
    assert_eq!(static_gain_db(-10.0, &duck, false), -12.0);
    assert_eq!(static_gain_db(-50.0, &duck, false), 0.0);
    assert!((transfer_db(-40.0, &s, false) + 50.0).abs() < 1e-4);
}

/// Through the DSP: a 2:1 expander 10 dB under its threshold settles 10 dB lower.
#[test]
fn an_expander_settles_on_its_curve() {
    let s = GateSettings {
        threshold_db: -20.0,
        ratio: 2.0,
        range_db: 60.0,
        attack_ms: 1.0,
        release_ms: 20.0,
        style: Style::Clean,
        ..GateSettings::default()
    };
    let x = sine(1_000.0, db(-30.0), 48_000, 0.0);
    let (l, _, _) = run(&s, &x, &x, None, 512);
    let got = 20.0 * math::log10(peak(&l[40_000..]));
    assert!(
        (got - -40.0).abs() < 0.5,
        "settled at {got} dB, expected −40"
    );
}

/// A signal hovering ±1.5 dB around the threshold: with no hysteresis the gate chatters;
/// with 3 dB it opens once and stays open.
#[test]
fn hysteresis_stops_chatter() {
    let n = ms(2_000.0);
    // 1 kHz at −30 dB, wobbling ±1.5 dB at 7 Hz and ±0.5 dB at 23 Hz.
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let wobble = 1.5 * math::sin(TAU * 7.0 * t) + 0.5 * math::sin(TAU * 23.0 * t);
            db(-30.0 + wobble * 0.75) * math::sin(TAU * 1_000.0 * t)
        })
        .collect();
    let count = |hyst: f32| {
        let s = GateSettings {
            threshold_db: -30.0,
            hysteresis_db: hyst,
            hold_ms: 0.0,
            ..settings(Style::Classic)
        };
        let trace = probe(&s, &x, None);
        let opens = trace
            .windows(2)
            .filter(|w| w[0].0 != Phase::Open && w[1].0 == Phase::Open)
            .count();
        let closes = trace
            .windows(2)
            .filter(|w| w[0].0 == Phase::Open && w[1].0 != Phase::Open)
            .count();
        (opens, closes)
    };
    let (chatter, _) = count(0.0);
    assert!(
        chatter >= 20,
        "without hysteresis it should chatter: {chatter} openings"
    );
    let (opens, closes) = count(3.0);
    assert_eq!(
        (opens, closes),
        (1, 0),
        "with 3 dB it opens once and stays open"
    );
}

/// Hold keeps the gate fully open for exactly the time set after the key falls below the
/// close threshold.
#[test]
fn hold_lasts_exactly_the_time_set() {
    for hold in [5.0f32, 50.0, 180.0] {
        let mut key = vec![0.5f32; ms(100.0)];
        key.extend(std::iter::repeat_n(0.0, ms(400.0)));
        let s = GateSettings {
            hold_ms: hold,
            ..settings(Style::Classic)
        };
        let trace = probe(&s, &key, Some(&key));
        let held: Vec<&(Phase, f32, f32)> = trace.iter().filter(|t| t.0 == Phase::Hold).collect();
        assert_eq!(held.len(), ms(hold), "hold {hold} ms");
        assert!(
            held.iter().all(|t| t.1 == 0.0),
            "fully open while holding, to the bit"
        );
    }
}

/// The time set is the time taken: from closed to within 1 % of open in the attack time,
/// from open to within 1 % of the range in the release time — in every style (Guitar's
/// closing follows the key instead; it has its own test).
#[test]
fn attack_and_release_take_the_time_set_in_every_style() {
    for style in [
        Style::Classic,
        Style::Clean,
        Style::Vocal,
        Style::Upward,
        Style::Ducking,
    ] {
        for (att, rel) in [(10.0f32, 100.0f32), (2.0, 400.0)] {
            let s = GateSettings {
                threshold_db: -30.0,
                hysteresis_db: 0.0,
                ratio: 100.0,
                range_db: 40.0,
                attack_ms: att,
                release_ms: rel,
                hold_ms: 0.0,
                ..settings(style)
            };
            // Quiet (fully settled), loud, quiet again.
            let mut key = vec![0.0f32; ms(1_000.0)];
            key.extend(std::iter::repeat_n(0.5, ms(1_000.0)));
            key.extend(std::iter::repeat_n(0.0, ms(2_000.0)));
            let x = vec![0.1f32; key.len()];
            let trace = probe(&s, &x, Some(&key));
            let swing = s.swing_db();
            let (open_db, closed_db) = match style.spec().mode {
                Mode::Down => (0.0, -swing),
                Mode::Up => (swing, 0.0),
                Mode::Duck => (-swing, 0.0),
            };
            let near = |g: f32, to: f32| (g - to).abs() <= 0.01 * swing;
            let start = trace.iter().position(|t| t.0 == Phase::Open).unwrap();
            let arrived = start
                + trace[start..]
                    .iter()
                    .position(|t| near(t.1, open_db))
                    .unwrap();
            let took = (arrived - start + 1) as f32 / SR * 1000.0;
            assert!(
                (took - att).abs() <= att * 0.03 + 0.05,
                "{style:?}: attack {att} ms took {took:.3} ms"
            );
            let opened = start;
            let start = opened
                + trace[opened..]
                    .iter()
                    .position(|t| t.0 == Phase::Release)
                    .unwrap();
            let arrived = start
                + trace[start..]
                    .iter()
                    .position(|t| near(t.1, closed_db))
                    .unwrap();
            let took = (arrived - start + 1) as f32 / SR * 1000.0;
            assert!(
                (took - rel).abs() <= rel * 0.03,
                "{style:?}: release {rel} ms took {took:.3} ms"
            );
        }
    }
}

/// Guitar lets a fading note down: while the key is still near the close threshold it
/// closes at a fraction of the speed Classic does; a note that stops is closed at once.
#[test]
fn guitar_follows_a_fading_note() {
    let n = ms(1_500.0);
    // Loud for 200 ms, then an exponential fade of 20 dB/s.
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let level = if t < 0.2 {
                -12.0
            } else {
                -12.0 - 20.0 * (t - 0.2)
            };
            db(level) * math::sin(TAU * 330.0 * t)
        })
        .collect();
    let slope = |style: Style| {
        let s = GateSettings {
            threshold_db: -20.0,
            hysteresis_db: 2.0,
            range_db: 60.0,
            release_ms: 100.0,
            hold_ms: 0.0,
            ..settings(style)
        };
        let trace = probe(&s, &x, None);
        let opened = trace.iter().position(|t| t.0 == Phase::Open).unwrap();
        let start = opened
            + trace[opened..]
                .iter()
                .position(|t| t.0 == Phase::Release)
                .unwrap();
        let later = start + ms(100.0);
        (trace[start].1 - trace[later].1) / 0.1 // dB per second, falling
    };
    let guitar = slope(Style::Guitar);
    let classic = slope(Style::Classic);
    assert!(classic > 500.0, "Classic fell at {classic:.1} dB/s");
    assert!(
        guitar < classic * 0.2,
        "Guitar fell at {guitar:.1} dB/s, Classic at {classic:.1}: it should let the note down"
    );
    // A note that stops is closed at the set speed: full cut within twice the release.
    let mut key = vec![0.5f32; ms(300.0)];
    key.extend(std::iter::repeat_n(0.0, ms(700.0)));
    let s = GateSettings {
        range_db: 60.0,
        release_ms: 100.0,
        hold_ms: 0.0,
        ..settings(Style::Guitar)
    };
    let trace = probe(&s, &key, Some(&key));
    let opened = trace.iter().position(|t| t.0 == Phase::Open).unwrap();
    let start = opened
        + trace[opened..]
            .iter()
            .position(|t| t.0 == Phase::Release)
            .unwrap();
    let shut = start + trace[start..].iter().position(|t| t.1 <= -59.4).unwrap();
    let took = (shut - start) as f32 / SR * 1000.0;
    assert!(
        took < 200.0,
        "a stopped note took {took:.0} ms to close once released"
    );
}

/// Opening is click-free: with the audio steady and the key stepping up, the energy the
/// opening splatters above 4 kHz stays far below the signal; a hard switch does not.
#[test]
fn opening_is_click_free() {
    let n = ms(300.0);
    // The key steps up at a crest of the audio: the worst moment to switch.
    let at = ms(150.0) + 60;
    let x = sine(200.0, 0.5, n, 0.0);
    let mut key = vec![0.0f32; n];
    key[at..].fill(0.5);
    // A 4 kHz high-pass, 48 dB/oct, run over what the gate made.
    let splatter = |s: &GateSettings| {
        let (l, _, _) = run(s, &x, &x, Some((&key, &key)), 64);
        let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
        let k = design(
            EqShape::LowCut,
            SR,
            4_000.0,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            48.0,
            &mut c,
        );
        let mut st = [Sec::default(); MAX_SECTIONS];
        let hf: Vec<f32> = l
            .iter()
            .map(|v| {
                let mut y = *v;
                for j in 0..k {
                    y = st[j].tick(&c[j], y);
                }
                y
            })
            .collect();
        20.0 * math::log10(peak(&hf[at - ms(5.0)..at + ms(40.0)]) / 0.5)
    };
    let base = GateSettings {
        range_db: 60.0,
        hysteresis_db: 0.0,
        ..GateSettings::default()
    };
    let clean = splatter(&GateSettings {
        attack_ms: 2.0,
        style: Style::Clean,
        ..base
    });
    let classic = splatter(&GateSettings {
        attack_ms: 2.0,
        style: Style::Classic,
        ..base
    });
    let hard = splatter(&GateSettings {
        attack_ms: 0.01,
        style: Style::Classic,
        ..base
    });
    // Measured: Clean −70.5 dB, Classic −42.2 dB, a hard switch −7.5 dB.
    assert!(clean < -65.0, "Clean splatters at {clean:.1} dB");
    assert!(classic < -38.0, "Classic splatters at {classic:.1} dB");
    assert!(hard > -15.0, "a hard switch should click: {hard:.1} dB");
}

/// Lookahead opens before the transient arrives, so the transient passes to the bit;
/// without it, the start of the hit is cut.
#[test]
fn lookahead_keeps_the_transient_whole() {
    let n = ms(200.0);
    let at = ms(100.0);
    let mut x = vec![0.0f32; n];
    // A hit that starts at its peak: the hardest edge there is.
    for (i, v) in x[at..].iter_mut().enumerate() {
        *v = 0.8
            * math::cos(TAU * 1_000.0 * i as f32 / SR + TAU / 4.0)
            * math::exp(-(i as f32) / SR * 20.0);
    }
    let s = GateSettings {
        attack_ms: 1.0,
        lookahead_ms: 5.0,
        style: Style::Clean,
        ..GateSettings::default()
    };
    let d = gate(s).latency();
    let (l, _, _) = run(&s, &x, &x, None, 128);
    assert_eq!(
        &l[at + d..at + d + ms(50.0)],
        &x[at..at + ms(50.0)],
        "the hit, delayed, whole"
    );
    let (l, _, _) = run(
        &GateSettings {
            lookahead_ms: 0.0,
            ..s
        },
        &x,
        &x,
        None,
        128,
    );
    let first = 20.0 * math::log10(l[at].abs() / x[at].abs());
    assert!(
        first < -20.0,
        "without lookahead the first sample is cut: {first:.1} dB"
    );
}

/// The key's filters decide what opens the gate: a high-pass at 1 kHz ignores a 100 Hz
/// thump as loud as a 3 kHz one that opens it.
#[test]
fn key_filters_decide_what_opens() {
    let s = GateSettings {
        threshold_db: -30.0,
        hpf_hz: 1_000.0,
        key_slope: 24.0,
        ..GateSettings::default()
    };
    let x = vec![0.1f32; ms(300.0)];
    let low = sine(100.0, db(-10.0), x.len(), 0.0);
    let high = sine(3_000.0, db(-10.0), x.len(), 0.0);
    let t = probe(&s, &x, Some(&low));
    assert!(
        t[ms(100.0)..].iter().all(|p| p.0 != Phase::Open),
        "100 Hz opened it"
    );
    let t = probe(&s, &x, Some(&high));
    assert!(
        t[ms(100.0)..].iter().all(|p| p.0 == Phase::Open),
        "3 kHz did not"
    );
    // The response drawn is the one heard: −3 dB at the corner, 24 dB an octave below.
    let at = key_filter_db(&s, SR, 1_000.0);
    assert!((at + 3.01).abs() < 0.1, "corner at {at} dB");
    let below = key_filter_db(&s, SR, 500.0);
    assert!((below + 24.1).abs() < 1.0, "an octave below at {below} dB");
    let lp = GateSettings {
        hpf_hz: 20.0,
        lpf_hz: 200.0,
        key_slope: 12.0,
        ..s
    };
    assert!((key_filter_db(&lp, SR, 200.0) + 3.01).abs() < 0.1);
    assert!(key_filter_db(&lp, SR, 3_000.0) < -40.0);
    let off = GateSettings::default();
    assert_eq!(key_filter_db(&off, SR, 50.0), 0.0, "off is flat");
}

/// Delta is what was taken away: gated plus delta is the input.
#[test]
fn delta_is_what_the_gate_removes() {
    let mut x = sine(250.0, 0.7, ms(500.0), 0.0);
    for v in &mut x[ms(200.0)..] {
        *v *= 0.001;
    }
    let s = GateSettings {
        lookahead_ms: 2.0,
        ..settings(Style::Clean)
    };
    let (gated, _, _) = run(&s, &x, &x, None, 100);
    let (delta, _, _) = run(
        &GateSettings {
            listen: Listen::Delta,
            ..s
        },
        &x,
        &x,
        None,
        100,
    );
    let d = gate(s).latency();
    for i in d..x.len() {
        assert!((gated[i] + delta[i] - x[i - d]).abs() < 1e-6, "at {i}");
    }
    assert!(peak(&delta[ms(260.0)..]) > 0.0, "the removed tail is heard");
}

/// Key listen plays the key the detector hears.
#[test]
fn key_listen_plays_the_key() {
    let x = sine(250.0, 0.7, 4_800, 0.0);
    let key = sine(3_000.0, 0.3, 4_800, 0.0);
    let s = GateSettings {
        listen: Listen::Key,
        ..GateSettings::default()
    };
    let (l, _, _) = run(&s, &x, &x, Some((&key, &key)), 64);
    assert_eq!(l, key);
}

/// Ducking: a loud key takes the range away, and it comes back when the key stops.
#[test]
fn ducking_lowers_the_signal_under_a_loud_key() {
    let n = ms(1_500.0);
    let x = sine(440.0, 0.5, n, 0.0);
    let mut key = sine(80.0, 0.8, ms(700.0), 0.0);
    key.extend(std::iter::repeat_n(0.0, n - key.len()));
    let s = GateSettings {
        threshold_db: -30.0,
        range_db: 12.0,
        ratio: 100.0,
        attack_ms: 5.0,
        release_ms: 200.0,
        ..settings(Style::Ducking)
    };
    let trace = probe(&s, &x, Some(&key));
    assert!(
        (trace[ms(600.0)].1 + 12.0).abs() < 0.01,
        "ducked by the range"
    );
    assert_eq!(trace[n - 1].1, 0.0, "and back to exactly 0 dB");
}

/// Upward raises what is above the threshold by the ratio.
#[test]
fn upward_raises_what_is_above() {
    let s = GateSettings {
        threshold_db: -30.0,
        ratio: 2.0,
        range_db: 12.0,
        ..settings(Style::Upward)
    };
    let x = sine(1_000.0, db(-24.0), 24_000, 0.0);
    let (l, _, _) = run(&s, &x, &x, None, 512);
    let got = 20.0 * math::log10(peak(&l[20_000..]));
    assert!(
        (got - -18.0).abs() < 0.3,
        "−24 dB, 6 over at 2:1 → −18: got {got}"
    );
}

// ---- telemetry and hits --------------------------------------------------------------

#[test]
fn telemetry_packs_exactly() {
    let c = Column {
        input_db: -12.3,
        output_db: -40.0,
        key_db: 3.5,
        gain_db: -100.0,
        phase: Phase::Hold,
    };
    let [a, b] = telemetry::pack(&c);
    assert!(a < (1 << 24) as f32 && b < (1 << 24) as f32);
    let back = telemetry::unpack(a, b);
    assert_eq!(back.phase, Phase::Hold);
    for (x, y) in [
        (back.input_db, c.input_db),
        (back.output_db, c.output_db),
        (back.key_db, c.key_db),
        (back.gain_db, c.gain_db),
    ] {
        assert!((x - y).abs() < 0.051, "{x} vs {y}");
    }
    assert_eq!(telemetry::since(3.0, (telemetry::WRAP - 2) as f32), 5);
}

/// One column per 5 ms, and each hit stamped where the key crossed the threshold — on the
/// input's timeline, lookahead or not.
#[test]
fn columns_count_time_and_hits_are_stamped_where_they_happened() {
    for look in [0.0f32, 4.0] {
        let n = ms(1_000.0);
        let mut x = vec![0.0f32; n];
        let hits = [ms(100.0), ms(400.0), ms(700.0)];
        for h in hits {
            for (j, v) in x[h..h + ms(50.0)].iter_mut().enumerate() {
                *v = 0.5
                    * math::sin(TAU * 200.0 * j as f32 / SR + TAU / 4.0)
                    * (1.0 - j as f32 / ms(50.0) as f32);
            }
        }
        let s = GateSettings {
            lookahead_ms: look,
            ..settings(Style::Classic)
        };
        let (_, _, t) = run(&s, &x, &x, None, 480);
        use telemetry::*;
        assert_eq!(t[SEQ], 200.0, "a second is 200 columns of 5 ms");
        assert_eq!(t[COLUMN_MS], 5.0);
        assert_eq!(t[HITS], 3.0);
        for (j, h) in hits.iter().rev().enumerate() {
            let ago = t[EVENTS_AT + 2 * j] as usize;
            assert_eq!(n - ago, *h, "hit {j} at lookahead {look}");
            let peak = t[EVENTS_AT + 2 * j + 1];
            assert!((peak - -6.02).abs() < 0.2, "peak {peak} dB");
        }
        // The last column is silence, closed; the one over the first hit is open.
        let last = unpack(t[COLUMNS_AT + 46], t[COLUMNS_AT + 47]);
        assert_eq!(last.phase, Phase::Closed);
        assert!(last.gain_db <= -39.9);
    }
}

// ---- learning ------------------------------------------------------------------------

fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed as f32 / u32::MAX as f32
}

/// Hits over bleed: it closes above the bleed and opens below the hits.
#[test]
fn learning_finds_the_gap_between_hits_and_bleed() {
    let mut seed = 9u32;
    let mut levels = Vec::new();
    for _ in 0..8 {
        // 40 columns of a hit decaying from −8 to −38 dB, then 60 of bleed at −45 ± 3.
        for j in 0..40 {
            levels.push(-8.0 - 30.0 * j as f32 / 40.0);
        }
        for _ in 0..60 {
            levels.push(-45.0 + 6.0 * (noise(&mut seed) - 0.5));
        }
    }
    let got = learn_thresholds(&levels, 4.0).unwrap();
    assert!(got.two_levels);
    assert!(
        got.close_db > -42.5 && got.close_db < -37.0,
        "close {}",
        got.close_db
    );
    assert!(got.open_db > got.close_db + 3.9, "open {}", got.open_db);
    assert!(
        got.open_db < -12.0,
        "every hit must still open it: {}",
        got.open_db
    );
}

/// Noise alone: it closes above the loudest 1 % and opens the hysteresis higher.
#[test]
fn learning_from_noise_alone_closes_above_it() {
    let mut seed = 5u32;
    let levels: Vec<f32> = (0..400).map(|_| -60.0 + 8.0 * noise(&mut seed)).collect();
    let got = learn_thresholds(&levels, 5.0).unwrap();
    assert!(!got.two_levels);
    assert!((got.close_db - -49.0).abs() < 1.0, "close {}", got.close_db);
    assert!((got.open_db - got.close_db - 5.0).abs() < 1e-4);
    assert!(learn_thresholds(&levels[..10], 5.0).is_none(), "too short");
    assert!(learn_thresholds(&[-200.0; 100], 5.0).is_none(), "silence");
}

#[test]
fn the_ramp_share_solves_its_equation() {
    assert_eq!(ramp_share(0.0), 1.0);
    for f in [0.02f32, 0.08, 0.12, 0.15, 0.18] {
        let c = ramp_share(f) as f64;
        let f = f as f64;
        let g = c + f * math::ln(100.0 * (f / c) * (1.0 - math::exp(-c / f)));
        assert!((g - 1.0).abs() < 1e-5, "f {f}: {g}");
    }
}

/// MIDI mode (ADR 0022): shut until the note's sample, open on it, and with lookahead
/// open exactly `look` before the note's audio comes out; the last note-off lets it hold
/// for its hold time and release; a second held note keeps it open.
#[test]
fn notes_open_it_on_their_sample_and_the_last_one_off_closes_it() {
    let base = GateSettings {
        midi: true,
        range_db: 100.0,
        attack_ms: 0.01,
        hold_ms: 5.0,
        release_ms: 10.0,
        ..GateSettings::default()
    };
    let n = ms(200.0);
    let x = vec![0.5f32; n];
    for (look, block) in [(0.0f32, 64usize), (3.0, 7), (3.0, 1000)] {
        let s = GateSettings {
            lookahead_ms: look,
            ..base
        };
        let mut p = gate(s);
        let (mut l, mut r) = (x.clone(), x.clone());
        let on = ms(20.0) + 3;
        let off = ms(100.0);
        let mut at = 0;
        while at < n {
            let end = (at + block).min(n);
            // The engine hands a note over before the call it falls in.
            for (t, trig) in [
                (
                    on,
                    Trigger::On {
                        pitch: 38,
                        velocity: 127,
                    },
                ),
                (
                    on + 10,
                    Trigger::On {
                        pitch: 40,
                        velocity: 90,
                    },
                ),
                (off - 500, Trigger::Off { pitch: 38 }),
                (off, Trigger::Off { pitch: 40 }),
            ] {
                if t >= at && t < end {
                    p.note(t - at, trig);
                }
            }
            p.process(&mut l[at..end], &mut r[at..end], None);
            at = end;
        }
        let lead = ms(look).min(p.latency());
        // Output sample k carries input k − latency; the note acts on the key's timeline,
        // `lead` samples ahead of the audio.
        let first = l.iter().position(|v| *v != 0.0).expect("it opened");
        assert_eq!(
            first + lead,
            on + p.latency(),
            "look {look} ms, block {block}: opened at {first}"
        );
        // Held through the first note-off (the second note is still down), closed after
        // the last one's hold and release.
        let lat = p.latency();
        assert!(
            l[off - 500 + lat + ms(8.0)] > 0.4,
            "the second note keeps it open"
        );
        let shut = off + lat - lead + ms(5.0) + ms(12.0);
        assert!(
            l[shut..].iter().all(|v| *v == 0.0),
            "look {look} ms: not shut after the last note's hold and release"
        );
    }
}

/// Round 28 (voice's review): more notes waiting at once than the queue holds lose no
/// note-off — the oldest acts early instead — so a held note's release always closes it.
#[test]
fn a_flood_past_the_queue_loses_no_note_off() {
    let mut p = gate(GateSettings {
        midi: true,
        range_db: 100.0,
        ..GateSettings::default()
    });
    p.note(
        0,
        Trigger::On {
            pitch: 72,
            velocity: 100,
        },
    );
    let (mut l, mut r) = (vec![0.5f32; 64], vec![0.5f32; 64]);
    p.process(&mut l, &mut r, None);
    assert_eq!(p.held_count, 1);
    // Every key released twice over, all on one sample: past the queue.
    for i in 0..MAX_TRIGGERS + 256 {
        p.note(
            10,
            Trigger::Off {
                pitch: (i % 128) as u8,
            },
        );
    }
    p.process(&mut l, &mut r, None);
    assert_eq!(p.held_count, 0, "a note-off was lost");
    assert_eq!(p.trigger_count, 0);
    assert!(p.held.iter().all(|h| !h));
}
