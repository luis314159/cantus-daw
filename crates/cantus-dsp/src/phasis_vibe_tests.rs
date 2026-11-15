//! The Uni-Vibe style (fx3:arca): the cells within their measured ranges, the stages
//! spread apart, the lamp's asymmetry, block size, rest, the null.

use super::vibe::{R_MAX, R_MEAN, R_MIN, STAGES};
use super::*;
use crate::modfx_lfo::Rate;

const SR: f32 = 48_000.0;

fn vibe(rate_hz: f64, mix: f32) -> PhasisSettings {
    let mut s = PhasisSettings {
        style: PhasisStyle::Vibe,
        mix,
        intensity: 1.0,
        ..PhasisSettings::default()
    };
    s.lfo.rate = Rate::Hz(rate_hz);
    s
}

fn fresh(s: &PhasisSettings) -> Phasis {
    let mut p = Phasis::new(SR);
    p.set(s);
    p.reset();
    p
}

fn noise(n: usize) -> Vec<f32> {
    let mut x = 0x1234_5678u32;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x as f32 / u32::MAX as f32 - 0.5) * 0.5
        })
        .collect()
}

/// Over a few cycles at full intensity: every cell inside its measured extremes, its
/// log-mean within a factor of two of the paper's measured mean; the four stages apart.
#[test]
fn the_cells_sweep_as_measured() {
    let mut p = fresh(&vibe(2.0, 1.0));
    let mut lo = [f32::MAX; STAGES];
    let mut hi = [0.0f32; STAGES];
    let mut ln_sum = [0.0f64; STAGES];
    let mut spread_min = f32::MAX;
    let n = 4 * 48_000;
    let block = 64;
    let mut count = 0usize;
    let (mut l, mut r) = (vec![0.0f32; block], vec![0.0f32; block]);
    for _ in 0..n / block {
        p.process(&mut l, &mut r);
        let mut fs = [0.0f32; STAGES];
        for k in 0..STAGES {
            let rk = p.vibe.resistance(0, k);
            lo[k] = lo[k].min(rk);
            hi[k] = hi[k].max(rk);
            ln_sum[k] += crate::math::ln(rk as f64);
            fs[k] = p.vibe.stage_hz(0, k);
        }
        count += 1;
        let (a, b) = fs.iter().fold((f32::MAX, 0.0f32), |(a, b), f| (a.min(*f), b.max(*f)));
        spread_min = spread_min.min(b / a);
    }
    for k in 0..STAGES {
        let mean = crate::math::exp(ln_sum[k] / count as f64) as f32;
        println!(
            "stage {k}: {:.1}k … {:.0}k, log-mean {:.0}k (measured {:.0}k)",
            lo[k] / 1e3,
            hi[k] / 1e3,
            mean / 1e3,
            R_MEAN[k] / 1e3
        );
        assert!(lo[k] >= R_MIN[k] * 0.999 && hi[k] <= R_MAX[k] * 1.001, "stage {k}");
        assert!(mean > 0.5 * R_MEAN[k] && mean < 2.0 * R_MEAN[k], "stage {k}: {mean}");
    }
    println!("the stages' frequencies always at least {spread_min:.0}× apart");
    assert!(spread_min > 10.0);
}

/// The light rises faster than it falls: over one slow cycle, the time from 10 % to 90 %
/// of its swing going up is shorter than going down.
#[test]
fn the_lamp_rises_fast_and_fades_slow() {
    let mut p = fresh(&vibe(1.0, 1.0));
    let (mut l, mut r) = (vec![0.0f32; 1], vec![0.0f32; 1]);
    for _ in 0..96_000 {
        p.process(&mut l, &mut r);
    }
    let light: Vec<f32> = (0..48_000)
        .map(|_| {
            p.process(&mut l, &mut r);
            p.vibe.lamps[0].light
        })
        .collect();
    let (mn, mx) = light.iter().fold((f32::MAX, 0.0f32), |(a, b), v| (a.min(*v), b.max(*v)));
    let at = |frac: f32| mn + (mx - mn) * frac;
    let imax = light.iter().position(|v| *v == mx).unwrap();
    let imin = light.iter().position(|v| *v == mn).unwrap();
    // Count the samples spent rising through 10–90 % and falling through it.
    let (mut up, mut down) = (0, 0);
    for w in light.windows(2) {
        let inside = w[1] > at(0.1) && w[1] < at(0.9);
        if inside && w[1] > w[0] {
            up += 1;
        } else if inside && w[1] < w[0] {
            down += 1;
        }
    }
    println!("light {mn:.3} … {mx:.3} (max at {imax}, min at {imin}): rising {up} samples, falling {down}");
    assert!(up < down, "{up} {down}");
}

#[test]
fn the_block_size_changes_nothing_and_rest_is_exact() {
    let x = noise(24_000);
    let mut outs = vec![];
    for block in [1, 37, 64, 512] {
        let mut p = fresh(&vibe(3.0, 0.5));
        let (mut l, mut r) = (x.clone(), x.clone());
        for (a, b) in l.chunks_mut(block).zip(r.chunks_mut(block)) {
            p.process(a, b);
        }
        outs.push(l);
    }
    for o in &outs[1..] {
        assert!(o.iter().zip(&outs[0]).all(|(a, b)| a.to_bits() == b.to_bits()));
    }
    // Twins through silence: one plays it, one skips it.
    let mk = || fresh(&vibe(3.0, 1.0));
    let (mut a, mut b) = (mk(), mk());
    for p in [&mut a, &mut b] {
        let (mut l, mut r) = (x.clone(), x.clone());
        p.process(&mut l, &mut r);
    }
    let mut blocks = 0;
    while !a.at_rest() {
        let (mut l, mut r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
        a.process(&mut l, &mut r);
        let (mut l2, mut r2) = (vec![0.0f32; 256], vec![0.0f32; 256]);
        b.process(&mut l2, &mut r2);
        blocks += 1;
        assert!(blocks < 1_000, "{:?}", a.rest_blocker());
    }
    let (mut l, mut r) = (vec![0.0f32; 7_000], vec![0.0f32; 7_000]);
    a.process(&mut l, &mut r);
    assert!(l.iter().all(|v| *v == 0.0));
    b.skip(7_000);
    let (mut la, mut ra) = (x.clone(), x.clone());
    let (mut lb, mut rb) = (x.clone(), x.clone());
    a.process(&mut la, &mut ra);
    b.process(&mut lb, &mut rb);
    assert!(la.iter().zip(&lb).all(|(p, q)| p.to_bits() == q.to_bits()));
    assert!(ra.iter().zip(&rb).all(|(p, q)| p.to_bits() == q.to_bits()));
}

/// At mix 0 the Uni-Vibe passes the dry untouched, as the classic does.
#[test]
fn the_vibe_at_mix_zero_is_a_null() {
    let x = noise(4_800);
    let mut p = fresh(&vibe(2.0, 0.0));
    let (mut l, mut r) = (x.clone(), x.clone());
    p.process(&mut l, &mut r);
    assert!(l.iter().zip(&x).all(|(a, b)| a.to_bits() == b.to_bits()));
}
