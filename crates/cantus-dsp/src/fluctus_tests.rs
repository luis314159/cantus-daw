use super::*;

const SR: f32 = 48_000.0;

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

fn unit(s: &FluctusSettings) -> Fluctus {
    let mut f = Fluctus::new(SR, holds);
    f.set(s);
    f.reset();
    f
}

fn playing(samples: i64) -> Transport {
    Transport {
        playing: true,
        tempo_bpm: 128.0,
        samples,
    }
}

fn drive(
    f: &mut Fluctus,
    l: &mut [f32],
    r: &mut [f32],
    start: i64,
    mut split: impl FnMut() -> usize,
) {
    let n = l.len();
    let mut at = 0;
    while at < n {
        let b = split().clamp(1, n - at);
        f.set_transport(&playing(start + at as i64));
        f.process(&mut l[at..at + b], &mut r[at..at + b]);
        at += b;
    }
}

fn pump(depth: f32, smooth: f32) -> FluctusSettings {
    FluctusSettings {
        depth,
        smooth_ms: smooth,
        ..FluctusSettings::default()
    }
}

fn point(x: f32, y: f32) -> Option<CurvePoint> {
    Some(CurvePoint {
        x,
        y,
        tension: 0.0,
        shape: 0,
    })
}

#[test]
fn at_depth_zero_it_is_a_wire() {
    let x = noise(20_000, 1);
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut unit(&pump(0.0, 5.0)), &mut l, &mut r, 0, || 333);
    assert!(same(&l, &x) && same(&r, &x));
    let (mut l, mut r) = (x.clone(), x.clone());
    unit(&FluctusSettings::default()).process(&mut l, &mut r);
    assert!(same(&l, &x) && same(&r, &x), "the defaults");
}

#[test]
fn the_curve_is_read_between_its_points_and_wraps() {
    let mut pts = [None; POINTS];
    pts[3] = point(0.75, 0.0);
    pts[0] = point(0.25, 1.0);
    let c = Curve::new(&pts, linear);
    assert_eq!(c.points()[0].x, 0.25, "sorted by place");
    assert_eq!(c.at(0.25), 1.0);
    assert!((c.at(0.5) - 0.5).abs() < 1e-6);
    assert_eq!(c.at(0.75), 0.0);
    // From 0.75 the curve runs to 0.25 a cycle on: 0 → 1 over half a cycle.
    assert!((c.at(0.0) - 0.5).abs() < 1e-6);
    assert!((c.at(0.875) - 0.25).abs() < 1e-6);
    assert_eq!(
        Curve::new(&[None; POINTS], linear).at(0.3),
        1.0,
        "no point: full"
    );
}

/// The pump at full depth, no smoothing: the level is nothing on each beat and full from
/// half way, every beat of the song.
#[test]
fn the_pump_ducks_on_every_beat_of_the_song() {
    let s = pump(1.0, 0.0);
    let beat = (60.0 / 128.0 * SR) as usize;
    let mut l = vec![1.0f32; 4 * beat];
    let mut r = l.clone();
    drive(&mut unit(&s), &mut l, &mut r, 0, || 512);
    for b in 0..4 {
        assert!(
            l[b * beat + 1] < 0.01,
            "beat {b} starts ducked: {}",
            l[b * beat + 1]
        );
        assert!(
            (l[b * beat + beat * 3 / 4] - 1.0).abs() < 1e-6,
            "beat {b} recovers"
        );
    }
}

#[test]
fn smoothing_takes_the_click_out_of_the_wrap() {
    let beat = (60.0 / 128.0 * SR) as usize;
    let jump = |smooth: f32| {
        let mut l = vec![1.0f32; 2 * beat];
        let mut r = l.clone();
        drive(&mut unit(&pump(1.0, smooth)), &mut l, &mut r, 0, || 512);
        l.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max)
    };
    assert!(jump(0.0) > 0.9, "without smoothing the wrap is a step");
    assert!(jump(2.0) < 0.02, "2 ms takes it out");
}

#[test]
fn it_is_the_same_at_every_split() {
    let s = FluctusSettings {
        depth: 0.9,
        beats: 0.25,
        offset: 0.3,
        smooth_ms: 4.0,
        out_db: -2.0,
        ..FluctusSettings::default()
    };
    let x = noise(30_000, 4);
    let (mut la, mut ra) = (x.clone(), x.clone());
    drive(&mut unit(&s), &mut la, &mut ra, 0, || 512);
    let mut k = 0usize;
    let (mut lb, mut rb) = (x.clone(), x.clone());
    drive(&mut unit(&s), &mut lb, &mut rb, 0, || {
        k += 1;
        [1, 7, 64, 4096, 129][k % 5]
    });
    assert!(same(&la, &lb) && same(&ra, &rb));
}

#[test]
fn skipping_is_processing_silence() {
    let mut a = unit(&pump(1.0, 10.0));
    assert!(a.at_rest());
    let mut b = a.clone();
    let (mut zl, mut zr) = (vec![0.0f32; 7_777], vec![0.0f32; 7_777]);
    a.process(&mut zl, &mut zr);
    assert!(zl.iter().chain(&zr).all(|v| v.to_bits() == 0));
    b.skip(7_777);
    let y = noise(9_000, 9);
    let (mut la, mut ra) = (y.clone(), y.clone());
    let (mut lb, mut rb) = (y.clone(), y);
    a.process(&mut la, &mut ra);
    b.process(&mut lb, &mut rb);
    assert!(same(&la, &lb) && same(&ra, &rb));
}

/// The automation's shapes as far as these tests need them: a hold (2) stays, the rest
/// run straight.
fn holds(shape: u8, _tension: f32, u: f32) -> f32 {
    if shape == 2 {
        0.0
    } else {
        u
    }
}

/// A render that starts at bar 5 — mid-cycle for a dotted quarter — with smoothing on and a
/// held curve: once the full render's glide has landed, the two are the same bits from the
/// first sample (the level starts on the curve, not gliding from phase 0).
#[test]
fn a_render_from_bar_five_mid_cycle_is_the_full_render_to_the_bit() {
    let bar5 = (16.0 * 60.0 / 128.0 * f64::from(SR)).round() as usize;
    let mut pts = [None; POINTS];
    pts[0] = Some(CurvePoint {
        x: 0.0,
        y: 0.2,
        tension: 0.0,
        shape: 2,
    });
    pts[1] = Some(CurvePoint {
        x: 0.4,
        y: 1.0,
        tension: 0.0,
        shape: 2,
    });
    let s = FluctusSettings {
        depth: 1.0,
        beats: 1.5,
        smooth_ms: 2.0,
        points: pts,
        ..FluctusSettings::default()
    };
    let x = noise(bar5 + 20_000, 7);
    let (mut lf, mut rf) = (x.clone(), x.clone());
    drive(&mut unit(&s), &mut lf, &mut rf, 0, || 480);
    let (mut lp, mut rp) = (x[bar5..].to_vec(), x[bar5..].to_vec());
    drive(&mut unit(&s), &mut lp, &mut rp, bar5 as i64, || 256);
    if let Some(i) = (0..lp.len()).find(|&i| lf[bar5 + i].to_bits() != lp[i].to_bits()) {
        panic!("first difference at {i}: {} vs {}", lf[bar5 + i], lp[i]);
    }
    assert!(same(&rf[bar5..], &rp));
}

#[test]
fn playback_from_mid_cycle_starts_on_the_curve() {
    let s = pump(1.0, 2.0);
    let mut live = unit(&s);
    // Two beats in (mid-cycle of a 1/1 length) after a while stopped.
    let mut s1 = s;
    s1.beats = 4.0;
    live.set(&s1);
    for _ in 0..20 {
        let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
        live.set_transport(&Transport {
            playing: false,
            tempo_bpm: 128.0,
            samples: 0,
        });
        live.process(&mut l, &mut r);
    }
    let beat = (60.0 / 128.0 * f64::from(SR)).round() as i64;
    let mut l = vec![1.0f32; 64];
    let mut r = l.clone();
    drive(&mut live, &mut l, &mut r, 2 * beat, || 64);
    // At half the cycle the pump is held open: full at once, no glide up from the duck.
    assert_eq!(l[0], 1.0);
}
