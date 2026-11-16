//! Purus's contract: the null, block-size independence, the delta, the latency, rest.

use super::testkit::{self, Noise};
use super::*;

const SR: f32 = 48_000.0;

/// A loud, busy program with hum on it: noise bursts, tones, the hum and buzz, silence.
fn program(seconds: f32, seed: u32) -> Vec<f32> {
    let mut rng = Noise::new(seed);
    let h = testkit::hum(SR, seconds, &[0.05, 0.02, 0.03, 0.01], 0.01, seed, |t| {
        50.0 + 0.4 * crate::math::sin(0.7 * t)
    });
    h.iter()
        .enumerate()
        .map(|(i, v)| {
            let t = i as f32 / SR;
            let burst = if (t * 3.0).fract() < 0.4 { 0.3 } else { 0.0 };
            v + burst * rng.next() + 0.2 * crate::math::sin(std::f32::consts::TAU * 220.0 * t)
        })
        .collect()
}

fn bits(x: &[f32]) -> Vec<u32> {
    x.iter().map(|v| v.to_bits()).collect()
}

fn hum_on() -> HumSettings {
    HumSettings {
        on: true,
        ..HumSettings::default()
    }
}

fn click_on() -> ClickSettings {
    ClickSettings {
        on: true,
        ..ClickSettings::default()
    }
}

fn verb_on() -> VerbSettings {
    VerbSettings {
        on: true,
        ..VerbSettings::default()
    }
}

/// The settings every contract test runs over.
fn heavy() -> Vec<PurusSettings> {
    vec![
        PurusSettings {
            hum: hum_on(),
            ..PurusSettings::default()
        },
        PurusSettings {
            hum: HumSettings {
                gentle: true,
                mode: HumMode::Track,
                harmonics: 16,
                ..hum_on()
            },
            ..PurusSettings::default()
        },
        PurusSettings {
            hum: HumSettings {
                mode: HumMode::Auto,
                mix: 0.6,
                out_db: -3.0,
                ..hum_on()
            },
            click: ClickSettings {
                crackle: 0.6,
                ..click_on()
            },
            verb: VerbSettings {
                decay: DecayMode::Estimate,
                amount_db: 24.0,
                ..verb_on()
            },
            out_db: 2.0,
        },
        PurusSettings {
            click: ClickSettings {
                sensitivity: 1.0,
                crackle: 1.0,
                mix: 0.7,
                ..click_on()
            },
            ..PurusSettings::default()
        },
        PurusSettings {
            verb: VerbSettings {
                smoothing: 0.0,
                keep_ms: 5.0,
                mix: 0.5,
                ..verb_on()
            },
            ..PurusSettings::default()
        },
    ]
}

fn run(s: &PurusSettings, x: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
    let mut p = Purus::new(SR);
    p.set(s);
    testkit::render(x, block, |l, r| p.process(l, r))
}

#[test]
fn every_module_off_is_a_null_to_the_bit_and_undelayed() {
    let x = program(2.0, 3);
    let s = PurusSettings::default();
    assert_eq!(latency_for(&s, SR), 0);
    let (l, r) = run(&s, &x, 64);
    assert_eq!(bits(&l), bits(&x));
    assert_eq!(bits(&r), bits(&x));
    // Settings moved but every module off: still the null.
    let off = PurusSettings {
        hum: HumSettings {
            depth_db: 80.0,
            gentle: true,
            ..HumSettings::default()
        },
        click: ClickSettings {
            sensitivity: 1.0,
            crackle: 1.0,
            ..ClickSettings::default()
        },
        verb: VerbSettings {
            amount_db: 30.0,
            ..VerbSettings::default()
        },
        out_db: 0.0,
    };
    let (l, _) = run(&off, &x, 37);
    assert_eq!(bits(&l), bits(&x));
}

#[test]
fn a_module_at_mix_zero_hands_its_input_on_to_the_bit() {
    let x = program(2.0, 5);
    // The de-hum's notches at mix 0: the input, undelayed.
    let s = PurusSettings {
        hum: HumSettings {
            mix: 0.0,
            ..hum_on()
        },
        ..PurusSettings::default()
    };
    assert_eq!(latency_for(&s, SR), 0);
    let (l, _) = run(&s, &x, 512);
    assert_eq!(bits(&l), bits(&x));
    // Its gentle mode at mix 0: the input, delayed by exactly its look-ahead (50 ms).
    let s = PurusSettings {
        hum: HumSettings {
            mix: 0.0,
            gentle: true,
            ..hum_on()
        },
        ..PurusSettings::default()
    };
    let lat = latency_for(&s, SR);
    assert_eq!(lat, 2400, "50 ms at 48 kHz");
    let (l, _) = run(&s, &x, 512);
    assert!(l[..lat].iter().all(|v| v.to_bits() == 0));
    assert_eq!(bits(&l[lat..]), bits(&x[..x.len() - lat]));
    // The de-click at mix 0, and on with nothing to find (a block without a click is handed
    // on as it came): the input, delayed by exactly its latency.
    for mix in [0.0, 1.0] {
        let s = PurusSettings {
            click: ClickSettings { mix, ..click_on() },
            ..PurusSettings::default()
        };
        let lat = latency_for(&s, SR);
        assert_eq!(lat, 767, "16 ms at 48 kHz");
        let quiet: Vec<f32> = if mix == 0.0 {
            x.clone()
        } else {
            (0..x.len())
                .map(|i| 0.3 * crate::math::sin(std::f32::consts::TAU * 220.0 * i as f32 / SR))
                .collect()
        };
        let (l, _) = run(&s, &quiet, 64);
        // (A tone starting out of silence is a click to the detector: past its start.)
        let from = lat + if mix == 0.0 { 0 } else { 2048 };
        assert_eq!(bits(&l[from..]), bits(&quiet[from - lat..quiet.len() - lat]), "mix {mix}");
    }
    // The de-reverb at mix 0: the input, delayed by exactly the latency stated.
    let s = PurusSettings {
        verb: VerbSettings {
            mix: 0.0,
            ..verb_on()
        },
        ..PurusSettings::default()
    };
    let lat = latency_for(&s, SR);
    assert_eq!(lat, 1023, "21.3 ms at 48 kHz");
    let (l, _) = run(&s, &x, 64);
    assert!(l[..lat].iter().all(|v| v.to_bits() == 0));
    assert_eq!(bits(&l[lat..]), bits(&x[..x.len() - lat]));
    // Both at mix 0, the de-reverb at amount 0 too.
    let s = PurusSettings {
        hum: HumSettings {
            mix: 0.0,
            ..hum_on()
        },
        click: ClickSettings {
            mix: 0.0,
            ..click_on()
        },
        verb: VerbSettings {
            mix: 0.0,
            amount_db: 0.0,
            ..verb_on()
        },
        out_db: 0.0,
    };
    let lat = latency_for(&s, SR);
    let (l, _) = run(&s, &x, 1);
    assert_eq!(bits(&l[lat..]), bits(&x[..x.len() - lat]));
}

#[test]
fn the_output_does_not_depend_on_the_block_size() {
    let x = program(3.0, 9);
    for s in heavy() {
        let want = run(&s, &x, 512);
        for block in [1usize, 37, 64] {
            let got = run(&s, &x, block);
            assert_eq!(bits(&got.0), bits(&want.0), "{s:?} at {block}");
            assert_eq!(bits(&got.1), bits(&want.1), "{s:?} at {block}");
        }
    }
    // Changes of every kind at the same sample, whatever the blocks.
    let changes = heavy();
    let at = [24_000usize, 48_000, 72_000, 100_000];
    let render = |block: usize| {
        let mut p = Purus::new(SR);
        let mut l = x.clone();
        let mut r = x.clone();
        let mut i = 0;
        let mut c = 0;
        while i < l.len() {
            if c < at.len() && i == at[c] {
                p.set(&changes[c]);
                c += 1;
            }
            let next = if c < at.len() { at[c] } else { l.len() };
            let e = (i + block).min(next).min(l.len());
            p.process(&mut l[i..e], &mut r[i..e]);
            i = e;
        }
        l
    };
    let want = render(512);
    for block in [1usize, 37, 64] {
        assert_eq!(bits(&render(block)), bits(&want), "changes at {block}");
    }
}

#[test]
fn the_delta_is_the_input_delayed_minus_the_output_to_the_bit() {
    let x = program(2.0, 11);
    for s in heavy() {
        let lat = latency_for(&s, SR);
        let (yl, yr) = run(&s, &x, 64);
        let mut d = s;
        d.hum.delta = true;
        d.click.delta = true;
        d.verb.delta = true;
        let (dl, dr) = run(&d, &x, 64);
        let g = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db)
        };
        for i in lat..x.len() {
            let xd = x[i - lat];
            // The output is the chain times the unit's level: undo it to compare.
            let (yl0, yr0) = if g == 1.0 {
                (yl[i], yr[i])
            } else {
                (yl[i] / g, yr[i] / g)
            };
            let want_l = if g == 1.0 { xd - yl0 } else { (xd - yl0) * g };
            let want_r = if g == 1.0 { xd - yr0 } else { (xd - yr0) * g };
            if g == 1.0 {
                assert_eq!(dl[i].to_bits(), want_l.to_bits(), "{s:?} at {i}");
                assert_eq!(dr[i].to_bits(), want_r.to_bits(), "{s:?} at {i}");
            } else {
                assert!((dl[i] - want_l).abs() <= 1e-6, "{s:?} at {i}: {} {want_l}", dl[i]);
            }
        }
    }
}

#[test]
fn one_modules_delta_is_what_that_module_takes_out() {
    let x = program(2.0, 13);
    let both = PurusSettings {
        hum: hum_on(),
        verb: verb_on(),
        ..PurusSettings::default()
    };
    let lat = latency_for(&both, SR);
    // The hum's delta: the input minus the hum's output, aligned to the output.
    let mut s = both;
    s.hum.delta = true;
    let (d, _) = run(&s, &x, 64);
    let only_hum = PurusSettings {
        hum: hum_on(),
        ..PurusSettings::default()
    };
    let (h, _) = run(&only_hum, &x, 64);
    for i in lat..x.len() {
        assert_eq!(
            d[i].to_bits(),
            (x[i - lat] - h[i - lat]).to_bits(),
            "at {i}"
        );
    }
    // The de-reverb's: the hum's output (delayed) minus the whole output.
    let mut s = both;
    s.verb.delta = true;
    let (d, _) = run(&s, &x, 64);
    let (y, _) = run(&both, &x, 64);
    for i in lat..x.len() {
        assert_eq!(d[i].to_bits(), (h[i - lat] - y[i]).to_bits(), "at {i}");
    }
}

#[test]
fn the_dereverb_reconstructs_its_input_when_it_takes_nothing() {
    // Amount 0 at full mix: the transform's round trip alone, delayed by the latency.
    let x = program(1.0, 17);
    let s = PurusSettings {
        verb: VerbSettings {
            amount_db: 0.0,
            ..verb_on()
        },
        ..PurusSettings::default()
    };
    let lat = latency_for(&s, SR);
    let (l, _) = run(&s, &x, 64);
    let mut worst = 0.0f32;
    for i in lat..x.len() {
        worst = worst.max((l[i] - x[i - lat]).abs());
    }
    assert!(worst < 2e-6, "round trip off by {worst}");
}

/// Sound, then silence: the unit must come to rest, and its twins agree.
#[test]
fn it_comes_to_rest_exactly_and_its_twins_agree() {
    let sound = program(1.5, 21);
    let after = program(1.0, 23);
    for s in heavy() {
        let mut a = Purus::new(SR);
        let mut b = Purus::new(SR);
        a.set(&s);
        b.set(&s);
        let block = 256;
        let feed = |p: &mut Purus, x: &[f32]| {
            let (l, r) = testkit::render(x, block, |l, r| p.process(l, r));
            (l, r)
        };
        feed(&mut a, &sound);
        feed(&mut b, &sound);
        assert!(!a.at_rest());
        // Silence until a rests; b is processed the same until then, and then skipped.
        let zeros = vec![0.0f32; block];
        let mut blocks = 0;
        while !a.at_rest() {
            let (l, _) = feed(&mut a, &zeros);
            let _ = feed(&mut b, &zeros);
            blocks += 1;
            assert!(blocks < 60 * 48_000 / block, "{s:?} never rests");
            if a.at_rest() {
                // The drain cuts nothing that sounds: the sample it came on is silent.
                let m = l[l.len() - 1].abs();
                assert!(
                    m < 1e-9,
                    "{s:?}: loud at the drain: {m} after {blocks} blocks"
                );
            }
        }
        assert!(b.at_rest());
        let (l, _) = feed(&mut a, &vec![0.0f32; 48_000]);
        assert!(l.iter().all(|v| v.to_bits() == 0), "{s:?}: silence at rest");
        b.skip(48_000);
        // And a reset twin and a new one (the decay estimated kept by reset, so it is
        // forgotten on both for the new one's comparison).
        let mut c = a.clone();
        c.reset();
        let ya = feed(&mut a, &after);
        let yb = feed(&mut b, &after);
        let yc = feed(&mut c, &after);
        assert_eq!(bits(&ya.0), bits(&yb.0), "{s:?}: skipped twin");
        assert_eq!(bits(&ya.0), bits(&yc.0), "{s:?}: reset twin");
        if s.verb.decay == DecayMode::Set {
            let mut d = Purus::new(SR);
            d.set(&s);
            let yd = feed(&mut d, &after);
            assert_eq!(bits(&ya.0), bits(&yd.0), "{s:?}: new twin");
        }
        let secs = blocks as f32 * block as f32 / SR;
        println!("{s:?}\n  at rest after {secs:.2} s of silence");
    }
}

#[test]
fn a_new_unit_is_at_rest_and_silence_keeps_it_there() {
    for s in heavy() {
        let mut p = Purus::new(SR);
        p.set(&s);
        assert!(p.at_rest());
        let mut l = vec![0.0f32; 4096];
        let mut r = vec![0.0f32; 4096];
        p.process(&mut l, &mut r);
        assert!(p.at_rest());
        assert!(l.iter().chain(r.iter()).all(|v| v.to_bits() == 0));
    }
}
