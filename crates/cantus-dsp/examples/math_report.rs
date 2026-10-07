//! Accuracy and speed of `cantus_dsp::math` against std's transcendentals (round 21,
//! `docs/round21/math.md`).
//!
//! ```text
//! python crates/cantus-dsp/examples/math_reference.py target/math_ref.txt
//! cargo run -p cantus-dsp --release --example math_report -- target/math_ref.txt
//! ```
//!
//! For every case of the reference file (a function, a type, the input range its call
//! sites use) it prints the worst and the mean error in ulps of each implementation, the
//! share of points where the two give the same bits, and nanoseconds per call (the best of
//! several passes, an indirect call's cost subtracted).

// std's functions are what this measures against.
#![allow(clippy::disallowed_methods)]

use cantus_dsp::math;
use std::hint::black_box;
use std::time::Instant;

type F32 = fn(f32, f32) -> f32;
type F64 = fn(f64, f64) -> f64;

/// `(std, cantus)` for one function at `f32`.
fn pair32(name: &str) -> (F32, F32) {
    match name {
        "sin" => (|x, _| x.sin(), |x, _| math::sin(x)),
        "cos" => (|x, _| x.cos(), |x, _| math::cos(x)),
        "tan" => (|x, _| x.tan(), |x, _| math::tan(x)),
        "exp" => (|x, _| x.exp(), |x, _| math::exp(x)),
        "exp2" => (|x, _| x.exp2(), |x, _| math::exp2(x)),
        "ln" => (|x, _| x.ln(), |x, _| math::ln(x)),
        "log2" => (|x, _| x.log2(), |x, _| math::log2(x)),
        "log10" => (|x, _| x.log10(), |x, _| math::log10(x)),
        "tanh" => (|x, _| x.tanh(), |x, _| math::tanh(x)),
        "atan" => (|x, _| x.atan(), |x, _| math::atan(x)),
        "sinh" => (|x, _| x.sinh(), |x, _| math::sinh(x)),
        "ln_1p" => (|x, _| x.ln_1p(), |x, _| math::ln_1p(x)),
        "cbrt" => (|x, _| x.cbrt(), |x, _| math::cbrt(x)),
        "powf" => (|x, y| x.powf(y), |x, y| math::powf(x, y)),
        "atan2" => (|x, y| x.atan2(y), |x, y| math::atan2(x, y)),
        "hypot" => (|x, y| x.hypot(y), |x, y| math::hypot(x, y)),
        other => panic!("no function {other}"),
    }
}

/// `(std, cantus)` for one function at `f64`.
fn pair64(name: &str) -> (F64, F64) {
    match name {
        "sin" => (|x, _| x.sin(), |x, _| math::sin(x)),
        "cos" => (|x, _| x.cos(), |x, _| math::cos(x)),
        "tan" => (|x, _| x.tan(), |x, _| math::tan(x)),
        "exp" => (|x, _| x.exp(), |x, _| math::exp(x)),
        "exp2" => (|x, _| x.exp2(), |x, _| math::exp2(x)),
        "ln" => (|x, _| x.ln(), |x, _| math::ln(x)),
        "log2" => (|x, _| x.log2(), |x, _| math::log2(x)),
        "log10" => (|x, _| x.log10(), |x, _| math::log10(x)),
        "tanh" => (|x, _| x.tanh(), |x, _| math::tanh(x)),
        "atan" => (|x, _| x.atan(), |x, _| math::atan(x)),
        "sinh" => (|x, _| x.sinh(), |x, _| math::sinh(x)),
        "ln_1p" => (|x, _| x.ln_1p(), |x, _| math::ln_1p(x)),
        "cbrt" => (|x, _| x.cbrt(), |x, _| math::cbrt(x)),
        "powf" => (|x, y| x.powf(y), |x, y| math::powf(x, y)),
        "atan2" => (|x, y| x.atan2(y), |x, y| math::atan2(x, y)),
        "hypot" => (|x, y| x.hypot(y), |x, y| math::hypot(x, y)),
        other => panic!("no function {other}"),
    }
}

/// One ulp at `r`'s magnitude, as an `f32` or as an `f64`.
fn ulp(r: f64, f32_ulp: bool) -> f64 {
    if f32_ulp {
        let b = (r as f32).abs().to_bits();
        let e = ((b >> 23) & 0xff) as i32;
        2f64.powi(if e == 0 { -149 } else { e - 127 - 23 })
    } else {
        let b = r.abs().to_bits();
        let e = ((b >> 52) & 0x7ff) as i32;
        2f64.powi(if e == 0 { -1074 } else { e - 1023 - 52 })
    }
}

fn err(v: f64, hi: f64, lo: f64, f32_ulp: bool) -> f64 {
    if !v.is_finite() || !hi.is_finite() {
        return if v == hi { 0.0 } else { f64::INFINITY };
    }
    ((v - hi) - lo).abs() / ulp(hi, f32_ulp)
}

struct Point {
    x: f64,
    y: f64,
    hi: f64,
    lo: f64,
}

/// The best of several passes, in ns per call.
fn time(n: usize, mut pass: impl FnMut() -> f64) -> f64 {
    let mut best = f64::INFINITY;
    let mut sink = 0.0;
    for _ in 0..7 {
        let t = Instant::now();
        for _ in 0..20 {
            sink += pass();
        }
        best = best.min(t.elapsed().as_secs_f64() * 1e9 / (20 * n) as f64);
    }
    black_box(sink);
    best
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/math_ref.txt".into());
    let text = std::fs::read_to_string(&path).expect("the reference file (math_reference.py)");
    let hex = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).expect("hex bits"));
    let mut cases: Vec<(String, String, Vec<Point>)> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('|').collect();
        let p = Point {
            x: hex(f[2]),
            y: hex(f[3]),
            hi: hex(f[4]),
            lo: hex(f[5]),
        };
        match cases.last_mut() {
            Some((c, t, v)) if c == f[0] && t == f[1] => v.push(p),
            _ => cases.push((f[0].into(), f[1].into(), vec![p])),
        }
    }
    println!("| Case | Type | std max | std mean | cantus max | cantus mean | same bits | std ns | cantus ns |");
    println!("|---|---|---:|---:|---:|---:|---:|---:|---:|");
    let id32: F32 = |x, _| x;
    let id64: F64 = |x, _| x;
    for (case, ty, pts) in &cases {
        let name = case.split(' ').next().expect("a name");
        let n = pts.len();
        let (mut e_std, mut e_can) = (Vec::with_capacity(n), Vec::with_capacity(n));
        let mut same = 0usize;
        let (t_std, t_can);
        if ty == "f32" {
            let (s, c) = pair32(name);
            let xs: Vec<(f32, f32)> = pts.iter().map(|p| (p.x as f32, p.y as f32)).collect();
            for (p, &(x, y)) in pts.iter().zip(&xs) {
                let (a, b) = (s(x, y), c(x, y));
                same += (a.to_bits() == b.to_bits()) as usize;
                e_std.push(err(a as f64, p.hi, p.lo, true));
                e_can.push(err(b as f64, p.hi, p.lo, true));
            }
            let run = |f: F32| {
                let f = black_box(f);
                time(n, || xs.iter().map(|&(x, y)| f(x, y) as f64).sum())
            };
            let base = run(id32);
            (t_std, t_can) = (run(s) - base, run(c) - base);
        } else {
            let (s, c) = pair64(name);
            for p in pts {
                let (a, b) = (s(p.x, p.y), c(p.x, p.y));
                same += (a.to_bits() == b.to_bits()) as usize;
                e_std.push(err(a, p.hi, p.lo, false));
                e_can.push(err(b, p.hi, p.lo, false));
            }
            let run = |f: F64| {
                let f = black_box(f);
                time(n, || pts.iter().map(|p| f(p.x, p.y)).sum())
            };
            let base = run(id64);
            (t_std, t_can) = (run(s) - base, run(c) - base);
        }
        let max = |v: &[f64]| v.iter().cloned().fold(0.0, f64::max);
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        println!(
            "| {case} | {ty} | {:.3} | {:.3} | {:.3} | {:.3} | {:.1} % | {:.1} | {:.1} |",
            max(&e_std),
            mean(&e_std),
            max(&e_can),
            mean(&e_can),
            100.0 * same as f64 / n as f64,
            t_std.max(0.0),
            t_can.max(0.0),
        );
    }
}
