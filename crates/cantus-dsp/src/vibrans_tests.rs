use super::*;
use crate::modfx_clock::{DIVISIONS, DIV_EIGHTH};

const SR: f32 = 48_000.0;

fn playing(samples: i64) -> Transport {
    Transport {
        playing: true,
        tempo_bpm: 128.0,
        samples,
    }
}

fn drive(v: &mut Vibrans, l: &mut [f32], r: &mut [f32], start: i64, mut split: impl FnMut() -> usize) {
    let n = l.len();
    let mut at = 0;
    while at < n {
        let b = split().clamp(1, n - at);
        v.set_transport(&playing(start + at as i64));
        v.process(&mut l[at..at + b], &mut r[at..at + b]);
        at += b;
    }
}

fn unit(s: &VibransSettings) -> Vibrans {
    let mut v = Vibrans::new(SR);
    v.set(s);
    v.reset();
    v
}

fn sine(f: f64, n: usize, level: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let a = (std::f64::consts::TAU * f * i as f64 / f64::from(SR)) % std::f64::consts::TAU;
            crate::math::sin(a) as f32 * level
        })
        .collect()
}

fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed | 1;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32 * 2.0 - 1.0) * 0.3
        })
        .collect()
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn busy() -> VibransSettings {
    VibransSettings {
        depth_cents: 40.0,
        lfo: LfoSettings {
            rate: Rate::Beats(DIVISIONS[DIV_EIGHTH]),
            shape: Shape::Triangle,
            ..LfoSettings::default()
        },
        rise_ms: 120.0,
        spread: 0.25,
        mix: 0.7,
        out_db: -1.0,
    }
}

/// The pitch, cents from `f0`, at each upward zero crossing after `from`.
fn cents_track(x: &[f32], f0: f64, from: usize) -> Vec<f64> {
    let mut out = Vec::new();
    let mut last: Option<f64> = None;
    for i in from.max(1)..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let t = (i - 1) as f64 + f64::from(-x[i - 1] / (x[i] - x[i - 1]));
            if let Some(p) = last {
                let f = f64::from(SR) / (t - p);
                out.push(1200.0 * crate::math::log2(f / f0));
            }
            last = Some(t);
        }
    }
    out
}

#[test]
fn at_depth_zero_it_is_a_wire_whatever_else_is_set() {
    let mut s = busy();
    s.depth_cents = 0.0;
    s.out_db = 0.0;
    s.mix = 1.0;
    let x = noise(20_000, 3);
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut unit(&s), &mut l, &mut r, 0, || 333);
    assert!(same(&l, &x) && same(&r, &x));
    let (mut l, mut r) = (x.clone(), x.clone());
    unit(&VibransSettings::default()).process(&mut l, &mut r);
    assert!(same(&l, &x) && same(&r, &x), "the defaults");
}

#[test]
fn it_is_the_same_at_every_split() {
    let x = noise(40_000, 5);
    let (mut la, mut ra) = (x.clone(), x.clone());
    drive(&mut unit(&busy()), &mut la, &mut ra, 0, || 512);
    let mut k = 0usize;
    let (mut lb, mut rb) = (x.clone(), x.clone());
    drive(&mut unit(&busy()), &mut lb, &mut rb, 0, || {
        k += 1;
        [1, 7, 64, 4096, 129][k % 5]
    });
    assert!(same(&la, &lb) && same(&ra, &rb));
}

#[test]
fn the_pitch_swings_by_the_cents_asked_for() {
    for (shape, cents, hz) in [(Shape::Sine, 50.0f32, 5.0f64), (Shape::Triangle, 25.0, 3.0), (Shape::Sine, 100.0, 0.5)] {
        let s = VibransSettings {
            depth_cents: cents,
            lfo: LfoSettings {
                rate: Rate::Hz(hz),
                shape,
                ..LfoSettings::default()
            },
            ..VibransSettings::default()
        };
        let x = sine(1_000.0, 4 * 48_000, 0.5);
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut unit(&s), &mut l, &mut r, 0, || 480);
        let track = cents_track(&l, 1_000.0, 12_000);
        let hi = track.iter().cloned().fold(f64::MIN, f64::max);
        let lo = track.iter().cloned().fold(f64::MAX, f64::min);
        let c = f64::from(cents);
        assert!((hi - c).abs() < 0.08 * c + 1.0, "{shape:?} {cents} ct at {hz} Hz: up {hi:.1}");
        assert!((lo + c).abs() < 0.08 * c + 1.0, "{shape:?} {cents} ct at {hz} Hz: down {lo:.1}");
    }
}

#[test]
fn the_depth_rises_after_a_silence() {
    let s = VibransSettings {
        depth_cents: 60.0,
        lfo: LfoSettings {
            rate: Rate::Hz(6.0),
            ..LfoSettings::default()
        },
        rise_ms: 500.0,
        ..VibransSettings::default()
    };
    // Silence, then a tone for a second.
    let mut x = vec![0.0f32; 9_600];
    x.extend(sine(1_000.0, 48_000, 0.5));
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut unit(&s), &mut l, &mut r, 0, || 256);
    let early = cents_track(&l[9_600..9_600 + 4_800], 1_000.0, 100);
    let late = cents_track(&l[9_600 + 30_000..], 1_000.0, 0);
    let swing = |t: &[f64]| t.iter().cloned().fold(0.0f64, |m, c| m.max(c.abs()));
    assert!(swing(&early) < 12.0, "the first 100 ms swing {}", swing(&early));
    assert!(swing(&late) > 50.0, "after the rise {}", swing(&late));
}

#[test]
fn a_synced_vibrato_from_bar_five_is_the_full_render_from_bar_five() {
    let bar5 = (16.0 * 60.0 / 128.0 * f64::from(SR)).round() as usize;
    let mut s = busy();
    s.rise_ms = 0.0;
    let x = noise(bar5 + 20_000, 6);
    let (mut lf, mut rf) = (x.clone(), x.clone());
    drive(&mut unit(&s), &mut lf, &mut rf, 0, || 480);
    // The late render needs the line's history: it starts with the same sound before bar 5.
    let pre = 9_600;
    let (mut lp, mut rp) = (x[bar5 - pre..].to_vec(), x[bar5 - pre..].to_vec());
    drive(&mut unit(&s), &mut lp, &mut rp, (bar5 - pre) as i64, || 256);
    assert!(same(&lf[bar5..], &lp[pre..]) && same(&rf[bar5..], &rp[pre..]));
}

#[test]
fn it_comes_to_rest_and_skipping_is_processing_silence() {
    let x = noise(10_000, 8);
    let (mut l, mut r) = (x.clone(), x.clone());
    let mut a = unit(&busy());
    drive(&mut a, &mut l, &mut r, 0, || 512);
    let mut silent = 0;
    while !a.at_rest() && silent < 96_000 {
        let (mut zl, mut zr) = (vec![0.0f32; 64], vec![0.0f32; 64]);
        a.process(&mut zl, &mut zr);
        silent += 64;
    }
    assert!(a.at_rest(), "{:?}", a.rest_blocker());
    let mut b = a.clone();
    let (mut zl, mut zr) = (vec![0.0f32; 999], vec![0.0f32; 999]);
    a.process(&mut zl, &mut zr);
    assert!(zl.iter().chain(&zr).all(|v| v.to_bits() == 0));
    b.skip(999);
    let y = noise(5_000, 9);
    let (mut la, mut ra) = (y.clone(), y.clone());
    let (mut lb, mut rb) = (y.clone(), y);
    a.process(&mut la, &mut ra);
    b.process(&mut lb, &mut rb);
    assert!(same(&la, &lb) && same(&ra, &rb));
}
