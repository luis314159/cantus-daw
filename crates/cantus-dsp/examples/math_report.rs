//! Accuracy and speed of `cantus_dsp::math` against std's transcendentals (round 21,
//! `docs/round21/math.md`) and against round 21's libm backend called directly (round 22,
//! `docs/round22/math2.md`: the `f32` `powf` of `DSP_VERSION` 3).
//!
//! ```text
//! python crates/cantus-dsp/examples/math_reference.py target/math_ref.txt
//! cargo run -p cantus-dsp --release --example math_report -- target/math_ref.txt
//! ```
//!
//! For every case of the reference file (a function, a type, the input range its call
//! sites use) it prints the worst and the mean error in ulps of each implementation — std
//! (the platform's C library), libm called directly (round 21's backend) and
//! `cantus_dsp::math` — the share of points where std and ours give the same bits, and
//! nanoseconds per call (the best of several passes, an indirect call's cost subtracted).
//! For `powf` in `f32` it adds one more candidate: `exp2(y · log2 x)` through libm's own
//! `f64` functions.

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
        "db" => (|x, _| 10f32.powf(x / 20.0), |x, _| math::db_to_gain(x)),
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
        "db" => (|x, _| 10f64.powf(x / 20.0), |x, _| math::powf(10.0, x / 20.0)),
        other => panic!("no function {other}"),
    }
}

/// Round 21's backend, called directly, at `f32`.
fn libm32(name: &str) -> F32 {
    match name {
        "sin" => |x, _| libm::sinf(x),
        "cos" => |x, _| libm::cosf(x),
        "tan" => |x, _| libm::tanf(x),
        "exp" => |x, _| libm::expf(x),
        "exp2" => |x, _| libm::exp2f(x),
        "ln" => |x, _| libm::logf(x),
        "log2" => |x, _| libm::log2f(x),
        "log10" => |x, _| libm::log10f(x),
        "tanh" => |x, _| libm::tanhf(x),
        "atan" => |x, _| libm::atanf(x),
        "sinh" => |x, _| libm::sinhf(x),
        "ln_1p" => |x, _| libm::log1pf(x),
        "cbrt" => |x, _| libm::cbrtf(x),
        "powf" => libm::powf,
        "db" => |x, _| libm::powf(10.0, x / 20.0),
        "atan2" => libm::atan2f,
        "hypot" => libm::hypotf,
        other => panic!("no function {other}"),
    }
}

/// Round 21's backend, called directly, at `f64`.
fn libm64(name: &str) -> F64 {
    match name {
        "sin" => |x, _| libm::sin(x),
        "cos" => |x, _| libm::cos(x),
        "tan" => |x, _| libm::tan(x),
        "exp" => |x, _| libm::exp(x),
        "exp2" => |x, _| libm::exp2(x),
        "ln" => |x, _| libm::log(x),
        "log2" => |x, _| libm::log2(x),
        "log10" => |x, _| libm::log10(x),
        "tanh" => |x, _| libm::tanh(x),
        "atan" => |x, _| libm::atan(x),
        "sinh" => |x, _| libm::sinh(x),
        "ln_1p" => |x, _| libm::log1p(x),
        "cbrt" => |x, _| libm::cbrt(x),
        "powf" => libm::pow,
        "db" => |x, _| libm::pow(10.0, x / 20.0),
        "atan2" => libm::atan2,
        "hypot" => libm::hypot,
        other => panic!("no function {other}"),
    }
}

/// The candidate round 21 recommended: `exp2(y · log2 x)` through libm's `f64` functions
/// (valid for the finite, positive bases of the reference's cases).
fn powf_by_libm_f64(x: f32, y: f32) -> f32 {
    libm::exp2(y as f64 * libm::log2(x as f64)) as f32
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
    // A true value past f32's range: the right answer is its rounding (∞).
    if f32_ulp && !(hi as f32).is_finite() {
        return if v == hi as f32 as f64 { 0.0 } else { f64::INFINITY };
    }
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


/// One row: errors (worst, mean) and ns per call of each implementation, std first.
struct Row {
    errs: Vec<(f64, f64)>,
    ns: Vec<f64>,
    same: f64,
}

fn stats(v: &[f64]) -> (f64, f64) {
    let max = v.iter().cloned().fold(0.0, f64::max);
    (max, v.iter().sum::<f64>() / v.len() as f64)
}

fn row32(pts: &[Point], fs: &[F32]) -> Row {
    let xs: Vec<(f32, f32)> = pts.iter().map(|p| (p.x as f32, p.y as f32)).collect();
    let outs: Vec<Vec<f32>> = fs
        .iter()
        .map(|f| xs.iter().map(|&(x, y)| f(x, y)).collect())
        .collect();
    let errs = outs
        .iter()
        .map(|o| {
            let e: Vec<f64> = o
                .iter()
                .zip(pts)
                .map(|(v, p)| err(*v as f64, p.hi, p.lo, true))
                .collect();
            stats(&e)
        })
        .collect();
    let last = outs.len() - 1;
    let same = outs[0]
        .iter()
        .zip(&outs[last])
        .filter(|(a, b)| a.to_bits() == b.to_bits())
        .count() as f64
        / pts.len() as f64;
    let run = |f: F32| {
        let f = black_box(f);
        time(pts.len(), || xs.iter().map(|&(x, y)| f(x, y) as f64).sum())
    };
    let base = run(|x, _| x);
    let ns = fs.iter().map(|&f| (run(f) - base).max(0.0)).collect();
    Row { errs, ns, same }
}

fn row64(pts: &[Point], fs: &[F64]) -> Row {
    let outs: Vec<Vec<f64>> = fs
        .iter()
        .map(|f| pts.iter().map(|p| f(p.x, p.y)).collect())
        .collect();
    let errs = outs
        .iter()
        .map(|o| {
            let e: Vec<f64> = o
                .iter()
                .zip(pts)
                .map(|(v, p)| err(*v, p.hi, p.lo, false))
                .collect();
            stats(&e)
        })
        .collect();
    let last = outs.len() - 1;
    let same = outs[0]
        .iter()
        .zip(&outs[last])
        .filter(|(a, b)| a.to_bits() == b.to_bits())
        .count() as f64
        / pts.len() as f64;
    let run = |f: F64| {
        let f = black_box(f);
        time(pts.len(), || pts.iter().map(|p| f(p.x, p.y)).sum())
    };
    let base = run(|x, _| x);
    let ns = fs.iter().map(|&f| (run(f) - base).max(0.0)).collect();
    Row { errs, ns, same }
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
    println!("| Case | Type | std max | std mean | libm max | libm mean | cantus max | cantus mean | std = cantus | std ns | libm ns | cantus ns |");
    println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    let mut extra = Vec::new();
    for (case, ty, pts) in &cases {
        let name = case.split(' ').next().expect("a name");
        let r = if ty == "f32" {
            let (s, c) = pair32(name);
            if name == "powf" {
                extra.push((
                    format!("{case} (libm f64 exp2∘log2)"),
                    row32(pts, &[s, libm32(name), powf_by_libm_f64]),
                ));
            }
            row32(pts, &[s, libm32(name), c])
        } else {
            let (s, c) = pair64(name);
            row64(pts, &[s, libm64(name), c])
        };
        println!(
            "| {case} | {ty} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.1} % | {:.1} | {:.1} | {:.1} |",
            r.errs[0].0, r.errs[0].1, r.errs[1].0, r.errs[1].1, r.errs[2].0, r.errs[2].1,
            100.0 * r.same, r.ns[0], r.ns[1], r.ns[2],
        );
    }
    println!("
The candidate round 21 recommended, in the cantus columns:
");
    for (case, r) in extra {
        println!(
            "| {case} | f32 | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.1} % | {:.1} | {:.1} | {:.1} |",
            r.errs[0].0, r.errs[0].1, r.errs[1].0, r.errs[1].1, r.errs[2].0, r.errs[2].1,
            100.0 * r.same, r.ns[0], r.ns[1], r.ns[2],
        );
    }
}
