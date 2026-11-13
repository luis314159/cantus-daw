//! Cribrum's own tests: the null, the same solver as the synths', the modes' responses,
//! self-oscillation in tune, the movement on the song, the curve's smoothing, vowels,
//! oversampling and rest.

use super::filter::{self, effective, Chan, Mode, Voice, MODELS};
use super::modulation::{self, CurveMod, Wave};
use super::response;
use super::*;
use crate::modfx_clock::Transport;
use crate::va::filter::{FilterCoefs, FilterSettings, VaFilter};
use crate::va::math::tan_pi;
use crate::va::testkit::{aliasing_floor_db, cents, frequency};

const SR: f32 = 48_000.0;

fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.wrapping_mul(747_796_405).wrapping_add(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32) * 1.2 - 0.6
        })
        .collect()
}

fn sine(n: usize, f: f32, a: f32) -> Vec<f32> {
    (0..n)
        .map(|i| a * crate::math::sin(std::f32::consts::TAU * f * i as f32 / SR))
        .collect()
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// Automation's shapes as far as these tests need them: a hold (2) stays, the rest run
/// straight.
fn holds(shape: u8, _tension: f32, u: f32) -> f32 {
    if shape == 2 {
        0.0
    } else {
        u
    }
}

fn unit(s: &CribrumSettings) -> Cribrum {
    let mut c = Cribrum::new(SR, holds);
    c.set(s);
    c.reset();
    c
}

fn playing(samples: i64) -> Transport {
    Transport {
        playing: true,
        tempo_bpm: 128.0,
        samples,
    }
}

/// Run `l`/`r` through `u` in blocks from `block`, the song playing from `start`.
fn drive(
    u: &mut Cribrum,
    l: &mut [f32],
    r: &mut [f32],
    start: i64,
    mut block: impl FnMut() -> usize,
) {
    let mut at = 0;
    while at < l.len() {
        let n = block().clamp(1, l.len() - at);
        u.set_transport(&playing(start + at as i64));
        u.process(&mut l[at..at + n], &mut r[at..at + n], None);
        at += n;
    }
}

/// A filter fully wet, at `cut` Hz.
fn wet(model: usize, mode: Mode, slope24: bool, cut: f32, res: f32) -> CribrumSettings {
    CribrumSettings {
        mix: 1.0,
        a: FilterParams {
            model: MODELS[model],
            mode,
            slope24,
            cut,
            res,
            ..FilterParams::default()
        },
        ..CribrumSettings::default()
    }
}

/// Everything moving: both filters, every source, oversampled, spread.
fn busy() -> CribrumSettings {
    let mut s = wet(1, Mode::Low, true, 800.0, 0.6);
    s.os = 2;
    s.spread = 0.5;
    s.a.drive_db = 9.0;
    s.b_on = true;
    s.b = FilterParams {
        model: Model::Svf,
        mode: Mode::High,
        cut: 120.0,
        res: 0.4,
        ..FilterParams::default()
    };
    s.b_track = -0.5;
    s.lfo = LfoParams {
        beats: 0.75,
        wave: Wave::Drift,
        stereo: 90.0,
        ..LfoParams::default()
    };
    s.lfo_depth = Depth {
        cut: 2.0,
        res: 0.2,
        vowel: 0.0,
    };
    s.env_depth = Depth {
        cut: 1.5,
        res: 0.0,
        vowel: 0.0,
    };
    s.curve_depth = Depth {
        cut: -1.0,
        res: 0.1,
        vowel: 0.0,
    };
    s.mix = 0.8;
    s.out_db = -2.0;
    s
}

// ---- the null --------------------------------------------------------------------------

#[test]
fn at_its_defaults_it_is_a_wire() {
    let mut u = unit(&CribrumSettings::default());
    let x = noise(4_096, 1);
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut u, &mut l, &mut r, 0, || 333);
    assert!(same(&l, &x) && same(&r, &x));
    assert!(u.at_rest());
    assert_eq!(u.latency(), 0);
}

#[test]
fn at_mix_zero_only_the_gain_is_heard() {
    let mut s = busy();
    s.mix = 0.0;
    s.os = 1;
    s.out_db = -6.0;
    let mut u = unit(&s);
    let x = noise(2_048, 2);
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut u, &mut l, &mut r, 0, || 512);
    let g = crate::math::db_to_gain(-6.0);
    assert!(l
        .iter()
        .zip(&x)
        .all(|(y, x)| y.to_bits() == (x * g).to_bits()));
}

// ---- the synths' filters --------------------------------------------------------------

/// The low-pass of every model is the synths' filter, sample for sample: the same solver
/// with the same settings (the four-poles at half their bass compensation).
#[test]
fn each_models_low_pass_is_the_synths_filter_to_the_bit() {
    let x = noise(4_000, 3);
    for (i, model) in MODELS.iter().enumerate() {
        for slope24 in [false, true] {
            let v = Voice {
                model: *model,
                mode: Mode::Low,
                slope24,
                cut: 900.0,
                res: 0.7,
                ..Voice::default()
            };
            let (_, s24) = effective(*model, Mode::Low, slope24);
            if *model == Model::Svf && s24 {
                continue; // the second stage is Cribrum's own
            }
            let fs = FilterSettings {
                model: *model,
                reso: filter::model_reso(0.7),
                poles: if *model == Model::Diode && !slope24 {
                    2
                } else {
                    4
                },
                morph: 1.0,
                bass_comp: if model.four_pole() { 0.5 } else { 0.0 },
                ..FilterSettings::default()
            };
            let c = FilterCoefs::new(&fs);
            let mut theirs = VaFilter::default();
            let mut ours = Chan::default();
            for (n, xv) in x.iter().enumerate() {
                let y = theirs.tick(*xv, tan_pi(900.0 / SR), &c, filter::SOLVE);
                let want = match model {
                    Model::Svf => theirs.taps()[1],
                    Model::Ladder | Model::Cascade if !slope24 => theirs.taps()[1],
                    _ => y,
                };
                let got = ours.tick(*xv, &v, SR);
                assert_eq!(
                    got.to_bits(),
                    want.to_bits(),
                    "model {i} slope24 {slope24} at {n}: {got} vs {want}"
                );
            }
        }
    }
}

/// A sine's gain through the filter, dB: `n` samples after a settle.
fn gain_db(v: &Voice, f: f32) -> f32 {
    let mut ch = Chan::default();
    let n = 9_600;
    let x = sine(n, f, 0.001);
    let y: Vec<f32> = x.iter().map(|s| ch.tick(*s, v, SR)).collect();
    let tail = n / 2;
    let rms = |v: &[f32]| {
        (v.iter().map(|s| f64::from(*s) * f64::from(*s)).sum::<f64>() / v.len() as f64).sqrt()
    };
    (20.0 * crate::math::log10(rms(&y[tail..]) / rms(&x[tail..]))) as f32
}

/// What the editor draws is what is heard: every model in every mode it gives, at both
/// slopes, measured with a quiet sine against [`response::response`] within 0.5 dB
/// wherever the response is above −40 dB.
#[test]
fn the_drawn_response_is_the_measured_one() {
    let mut worst = 0f32;
    for model in MODELS {
        for mode in Mode::ALL {
            for slope24 in [false, true] {
                for res in [0.0, 0.5, 0.85] {
                    let v = Voice {
                        model,
                        mode,
                        slope24,
                        cut: 1_200.0,
                        res,
                        vowel: 1.5,
                        ..Voice::default()
                    };
                    for f in [150.0, 600.0, 1_200.0, 2_400.0, 6_000.0] {
                        let drawn =
                            20.0 * crate::math::log10(response::response(&v, f, SR).abs()) as f32;
                        if drawn < -40.0 {
                            continue;
                        }
                        let heard = gain_db(&v, f);
                        let err = (heard - drawn).abs();
                        worst = worst.max(err);
                        assert!(
                            err < 0.5,
                            "{model:?} {mode:?} 24:{slope24} res {res} at {f} Hz: heard {heard:.2} dB, drawn {drawn:.2} dB"
                        );
                    }
                }
            }
        }
    }
    println!("worst difference between the drawn and the heard response: {worst:.3} dB");
}

// ---- self-oscillation ------------------------------------------------------------------

/// At full resonance every model rings on its cutoff after a kick, within ±5 cents, at 1×
/// and oversampled — through the whole unit, so the oversampler's phase is in it too — and
/// stays bounded.
#[test]
fn at_full_resonance_every_model_sings_on_its_cutoff() {
    let mut worst = 0f64;
    for os in [1usize, 2] {
        for (i, model) in MODELS.iter().enumerate() {
            let mut line = format!("{os}x {:>10}:", crate::va::filter::describe(*model));
            for fc in [110.0f32, 440.0, 1_000.0, 2_500.0] {
                let mut s = wet(i, Mode::Low, true, fc, 1.0);
                s.os = os;
                let mut u = unit(&s);
                let n = (SR * 3.0) as usize;
                let mut l = vec![0.0f32; n];
                l[0] = 0.5;
                let mut r = l.clone();
                drive(&mut u, &mut l, &mut r, 0, || 512);
                let tail = &l[n - n / 6..];
                let f = frequency(tail, SR).unwrap_or_else(|| panic!("{model:?} {fc} silent"));
                let err = cents(f, f64::from(fc));
                worst = worst.max(err.abs());
                line.push_str(&format!(" {fc:>5}Hz {err:+5.1}¢"));
                let peak = tail.iter().fold(0f32, |m, v| m.max(v.abs()));
                assert!(
                    peak < 4.0 && peak > 0.01,
                    "{model:?} at {fc} Hz: amplitude {peak}"
                );
                assert!(
                    err.abs() <= 5.0,
                    "{os}x {model:?} self-oscillates {err:+.2} cents off {fc} Hz"
                );
            }
            println!("{line}");
        }
    }
    println!("worst self-oscillation error: {worst:.2} cents");
}

// ---- the movement on the song ---------------------------------------------------------

/// The cutoff a busy setting moves to, sample by sample, from song sample `start`.
fn cutoffs(s: &CribrumSettings, x: &[f32], start: i64) -> Vec<f32> {
    let mut u = unit(s);
    let (mut l, mut r) = (x.to_vec(), x.to_vec());
    let mut out = Vec::with_capacity(x.len());
    let mut tel = [0.0f32; 64];
    for i in 0..x.len() {
        u.set_transport(&playing(start + i as i64));
        u.process(&mut l[i..i + 1], &mut r[i..i + 1], None);
        u.telemetry(&mut tel);
        out.push(tel[t::CUT_A_L]);
    }
    out
}

/// A synced sweep (LFO and curve, no follower) rendered from bar 5 moves the cutoff as bar
/// 5 of the whole render does, to the bit, once the glides have landed; the audio, whose
/// filters remember what came before, agrees within −100 dB after 50 ms.
#[test]
fn a_render_from_bar_five_moves_as_bar_five_of_the_whole_render() {
    let bar5 = (16.0 * 60.0 / 128.0 * f64::from(SR)).round() as usize;
    let mut s = busy();
    s.env_depth = Depth::default();
    let n = 12_000;
    let x = noise(bar5 + n, 4);
    let whole = cutoffs(&s, &x, 0);
    let part = cutoffs(&s, &x[bar5..], bar5 as i64);
    // The glides start where they are set: from the first sample.
    if let Some(i) = (0..n).find(|&i| whole[bar5 + i].to_bits() != part[i].to_bits()) {
        panic!(
            "the cutoff differs at {i}: {} vs {}",
            whole[bar5 + i],
            part[i]
        );
    }
    let (mut lf, mut rf) = (x.clone(), x.clone());
    drive(&mut unit(&s), &mut lf, &mut rf, 0, || 480);
    let (mut lp, mut rp) = (x[bar5..].to_vec(), x[bar5..].to_vec());
    drive(&mut unit(&s), &mut lp, &mut rp, bar5 as i64, || 256);
    let settle = (0.05 * SR) as usize;
    let worst = (settle..n)
        .map(|i| {
            (lf[bar5 + i] - lp[i])
                .abs()
                .max((rf[bar5 + i] - rp[i]).abs())
        })
        .fold(0f32, f32::max);
    assert!(worst < 1e-5, "after 50 ms the renders differ by {worst}");
}

/// The same settings give the same bits at every block size, with every source moving.
#[test]
fn it_is_the_same_at_every_split() {
    let s = busy();
    let x = noise(20_000, 5);
    let y = noise(20_000, 6);
    let run = |b: usize| {
        let (mut l, mut r) = (x.clone(), y.clone());
        drive(&mut unit(&s), &mut l, &mut r, 1_000, || b);
        (l, r)
    };
    let (l0, r0) = run(512);
    for b in [1usize, 37, 64, 4_096] {
        let (l, r) = run(b);
        assert!(same(&l, &l0) && same(&r, &r0), "blocks of {b}");
    }
}

/// A free LFO starts its cycle from the start of playback, a synced one from the song: the
/// value at a song position is the same whenever it is reached.
#[test]
fn the_synced_lfo_is_a_function_of_the_song() {
    let mut s = wet(0, Mode::Low, false, 500.0, 0.2);
    s.lfo_depth.cut = 3.0;
    s.lfo.beats = 1.0;
    let x = vec![0.1f32; 4_000];
    let a = cutoffs(&s, &x, 22_500);
    let b = cutoffs(&s, &x[..1_000], 25_500);
    assert!(same(&a[3_000..], &b));
    // A quarter of 128 BPM is 22 500 samples: the sine's start (its middle) on the beat.
    assert!((a[0] - 500.0).abs() < 0.5, "{}", a[0]);
}

// ---- the curve's smoothing ------------------------------------------------------------

/// A curve of holds: up for the first half, down for the second.
fn steps() -> [Option<CurvePoint>; POINTS] {
    let mut pts = [None; POINTS];
    pts[0] = Some(CurvePoint {
        x: 0.0,
        y: 1.0,
        tension: 0.0,
        shape: 2,
    });
    pts[1] = Some(CurvePoint {
        x: 0.5,
        y: 0.0,
        tension: 0.0,
        shape: 2,
    });
    pts
}

/// The largest move of the curve from one sample to the next, over two cycles.
fn largest_step(smooth_ms: f32) -> f32 {
    let mut c = CurveMod::new(SR, holds);
    c.set(&steps(), holds, 0.5, smooth_ms);
    let mut last = c.at(0, 128.0);
    let mut worst = 0f32;
    for t in 1..(2 * 11_250) {
        let v = c.at(t, 128.0);
        worst = worst.max((v - last).abs());
        last = v;
    }
    worst
}

/// A held step jumps in one sample with no smoothing; at the default 2 ms no sample moves
/// more than 3 % of the step, and the slope grows gentler as the smoothing grows.
#[test]
fn the_curves_steps_never_click() {
    assert_eq!(largest_step(0.0), 1.0);
    let at2 = largest_step(2.0);
    assert!(at2 < 0.03, "2 ms: a sample moves {at2}");
    let at10 = largest_step(10.0);
    assert!(at10 < at2 / 3.0, "10 ms: {at10} against 2 ms: {at2}");
    // Smoothing a held level changes nothing: the middle of each half is the curve.
    let mut c = CurveMod::new(SR, holds);
    c.set(&steps(), holds, 0.5, 2.0);
    assert_eq!(c.at(2_800, 128.0), 1.0);
    assert_eq!(c.at(8_400, 128.0), 0.0);
}

/// Heard: a low sine through a low-pass under it whose cutoff the stepped curve throws
/// four octaves up and back each eighth (its level jumps fourfold): smoothed, the output's
/// largest kink is a small part of the unsmoothed one.
#[test]
fn a_stepped_sweep_is_heard_without_a_click() {
    let run = |smooth: f32| {
        let mut s = wet(0, Mode::Low, false, 100.0, 0.5);
        s.curve = CurveParams {
            beats: 0.5,
            smooth_ms: smooth,
            points: steps(),
        };
        s.curve_depth.cut = 4.0;
        let x = sine(24_000, 200.0, 0.5);
        let (mut l, mut r) = (x.clone(), x);
        drive(&mut unit(&s), &mut l, &mut r, 0, || 256);
        // The second difference: a click is a kink.
        (2..l.len())
            .map(|i| (l[i] - 2.0 * l[i - 1] + l[i - 2]).abs())
            .fold(0f32, f32::max)
    };
    let smooth = run(2.0);
    let raw = run(0.0);
    println!("largest kink: smoothed {smooth:.4}, unsmoothed {raw:.4}");
    assert!(smooth < raw / 4.0, "{smooth} vs {raw}");
}

// ---- vowels ----------------------------------------------------------------------------

/// The vowels' formants are where the table puts them: the response peaks at each of A's
/// three, and I's second formant sits far above A's.
#[test]
fn the_vowels_peak_at_their_formants() {
    let at = |vowel: f32, f: f32| {
        let v = Voice {
            mode: Mode::Vowel,
            vowel,
            ..Voice::default()
        };
        response::response(&v, f, SR).abs()
    };
    for (f, _, _) in filter::FORMANTS[0] {
        assert!(
            at(0.0, f) > at(0.0, f * 1.25) && at(0.0, f) > at(0.0, f / 1.25),
            "{f}"
        );
    }
    assert!(at(2.0, 1_870.0) > 4.0 * at(2.0, 1_080.0));
    // Between two vowels the formants lie between theirs.
    let mid = filter::formants(0.5, 1_000.0, 0.25);
    assert!(mid[1].0 > 1_080.0 && mid[1].0 < 1_700.0);
    // The cutoff scales them all.
    let up = filter::formants(0.0, 2_000.0, 0.25);
    assert!((up[0].0 - 1_300.0).abs() < 1.0);
}

/// On pink-ish noise the vowel filter sits near the dry level (its makeup is measured).
#[test]
fn a_vowel_keeps_the_level() {
    let x = noise(48_000, 9);
    // Pink-ish: a one-pole low-pass on white noise.
    let mut z = 0.0f32;
    let x: Vec<f32> = x
        .iter()
        .map(|v| {
            z += (v - z) * 0.05;
            z * 4.0
        })
        .collect();
    let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
    for vowel in [0.0f32, 1.0, 2.0, 3.0, 4.0] {
        let mut s = wet(0, Mode::Vowel, false, 1_000.0, 0.25);
        s.a.vowel = vowel;
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut unit(&s), &mut l, &mut r, 0, || 512);
        let db = 20.0 * crate::math::log10(rms(&l[4_800..]) / rms(&x[4_800..]));
        println!("vowel {vowel}: {db:+.1} dB");
        assert!(db.abs() < 4.0, "vowel {vowel}: {db:+.1} dB");
    }
}

// ---- oversampling ----------------------------------------------------------------------

/// Oversampled, the latency is reported and the dry path waits for it: half wet, a
/// low-pass far above a low sine sums with the dry without a comb.
#[test]
fn the_dry_path_waits_for_the_oversampling() {
    for os in [2usize, 4] {
        let mut s = wet(0, Mode::Low, false, 18_000.0, 0.0);
        s.os = os;
        s.mix = 0.5;
        let mut u = unit(&s);
        assert!(u.latency() >= 1, "{os}x reports no latency");
        let x = sine(9_600, 200.0, 0.5);
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut u, &mut l, &mut r, 0, || 512);
        let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
        let db = 20.0 * crate::math::log10(rms(&l[4_800..]) / rms(&x[4_800..]));
        assert!(db.abs() < 0.1, "{os}x half wet: {db:+.3} dB");
    }
}

/// Driven hard, the filter's harmonics fold back less when oversampled: the aliasing floor
/// of a 5 kHz sine through a driven ladder falls at 2× and again at 4× (measured, printed).
#[test]
fn oversampling_lowers_the_aliasing_of_a_driven_filter() {
    let mut floors = Vec::new();
    for os in [1usize, 2, 4] {
        let mut s = wet(1, Mode::Low, true, 16_000.0, 0.5);
        s.a.drive_db = 24.0;
        s.os = os;
        let x = sine(16_384 + 4_096, 4_987.5, 0.9);
        let (mut l, mut r) = (x.clone(), x);
        drive(&mut unit(&s), &mut l, &mut r, 0, || 512);
        let floor = aliasing_floor_db(&l[4_096..], SR, 4_987.5, 200.0, 20_000.0);
        println!("{os}x: aliasing floor {floor:.1} dB");
        floors.push(floor);
    }
    assert!(floors[1] < floors[0] - 6.0, "{floors:?}");
    assert!(floors[2] < floors[1], "{floors:?}");
}

// ---- rest ------------------------------------------------------------------------------

/// After sound, silence brings it to rest (every filter as new), where a new twin skipped
/// as far renders the same bits as it does.
#[test]
fn it_comes_to_rest_and_is_then_as_good_as_new() {
    let s = busy();
    let mut a = unit(&s);
    let x = noise(9_000, 10);
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut a, &mut l, &mut r, 0, || 256);
    let mut silent = 0usize;
    while !a.at_rest() {
        let (mut l, mut r) = (vec![0.0f32; 512], vec![0.0f32; 512]);
        drive(&mut a, &mut l, &mut r, (9_000 + silent) as i64, || 512);
        silent += 512;
        assert!(silent < 30 * 48_000, "no rest: {:?}", a.rest_blocker());
    }
    let mut b = unit(&s);
    b.skip(9_000 + silent);
    let y = noise(6_000, 11);
    let (mut la, mut ra) = (y.clone(), y.clone());
    let (mut lb, mut rb) = (y.clone(), y);
    drive(&mut a, &mut la, &mut ra, (9_000 + silent) as i64, || 300);
    drive(&mut b, &mut lb, &mut rb, (9_000 + silent) as i64, || 300);
    assert!(same(&la, &lb) && same(&ra, &rb));
}

/// A filter singing on its own never claims rest.
#[test]
fn a_self_oscillating_filter_is_never_at_rest() {
    let s = wet(0, Mode::Low, false, 440.0, 1.0);
    let mut u = unit(&s);
    let (mut l, mut r) = (vec![0.0f32; 48_000], vec![0.0f32; 48_000]);
    l[0] = 0.5;
    r[0] = 0.5;
    drive(&mut u, &mut l, &mut r, 0, || 512);
    assert!(!u.at_rest());
}

// ---- the follower ----------------------------------------------------------------------

/// The follower opens the filter as the input grows; with `env.key` it hears the sidechain
/// instead (and the input, when none is routed).
#[test]
fn the_follower_moves_the_cutoff_with_the_level() {
    let mut s = wet(0, Mode::Low, false, 200.0, 0.3);
    s.env_depth.cut = 4.0;
    let cut_after = |s: &CribrumSettings, x: f32, key: Option<f32>| {
        let mut u = unit(s);
        let mut tel = [0.0f32; 64];
        for _ in 0..20 {
            let (mut l, mut r) = (vec![x; 480], vec![x; 480]);
            let k = key.map(|k| vec![k; 480]);
            u.process(&mut l, &mut r, k.as_ref().map(|k| (&k[..], &k[..])));
        }
        u.telemetry(&mut tel);
        tel[t::CUT_A_L]
    };
    let quiet = cut_after(&s, 0.003, None);
    let loud = cut_after(&s, 0.9, None);
    assert!(
        (quiet - 200.0).abs() < 0.1 && loud > 2_500.0,
        "{quiet} {loud}"
    );
    s.env.key = true;
    assert!(cut_after(&s, 0.9, Some(0.0)) < 210.0);
    assert!(cut_after(&s, 0.0, Some(0.9)) > 2_500.0);
    assert!(cut_after(&s, 0.9, None) > 2_500.0);
}

/// The follower's map: nothing below its range (−48 dBFS by default), all of it at full
/// scale.
#[test]
fn the_followers_level_maps_in_decibels() {
    assert_eq!(modulation::env_amount(0.0, 48.0), 0.0);
    assert_eq!(modulation::env_amount(0.001, 48.0), 0.0);
    assert!((modulation::env_amount(1.0, 48.0) - 1.0).abs() < 1e-6);
    let half = modulation::env_amount(crate::math::db_to_gain(-24.0), 48.0);
    assert!((half - 0.5).abs() < 1e-4, "{half}");
    // A narrow range: the movement spans 12 dB only.
    let narrow = modulation::env_amount(crate::math::db_to_gain(-6.0), 12.0);
    assert!((narrow - 0.5).abs() < 1e-4, "{narrow}");
    assert_eq!(
        modulation::env_amount(crate::math::db_to_gain(-13.0), 12.0),
        0.0
    );
}
