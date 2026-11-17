//! What each step of a formula computes (ADR 0148): `cantus_dsp::math` for every
//! transcendental, FL's definitions where its manual gives them, no loop longer than a
//! factorial's 170 steps.

use super::{Env, Func, Op, Var};
use cantus_dsp::math;
use std::f64::consts::PI;

#[inline]
fn truth(x: f64) -> bool {
    x > 0.5
}

#[inline]
fn bit(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
}

/// The fractional part, Delphi's way: `x − Int(x)` (negative for a negative `x`).
#[inline]
fn frac(x: f64) -> f64 {
    x - x.trunc()
}

/// Where in its cycle `x` is, 0…1.
#[inline]
fn cycle(x: f64) -> f64 {
    x - x.floor()
}

pub(super) fn var(v: Var, env: &Env) -> f64 {
    match v {
        Var::A => env.a,
        Var::B => env.b,
        Var::C => env.c,
        Var::Beats => env.beats,
        Var::Seconds => env.seconds,
        Var::Millis => env.seconds * 1000.0,
        Var::Tempo => env.tempo,
        Var::Pi => PI,
        Var::E => std::f64::consts::E,
        Var::Rand => {
            let h = cantus_dsp::modfx_clock::hash64(env.beats.to_bits() ^ 0x5EED_F0E1_D0C0_4A11);
            (h >> 11) as f64 / (1u64 << 53) as f64
        }
    }
}

pub(super) fn unary(op: Op, x: f64) -> f64 {
    match op {
        Op::Neg => -x,
        Op::Not => bit(!truth(x)),
        Op::Percent => x / 100.0,
        Op::Factorial => factorial(x),
        _ => f64::NAN,
    }
}

/// `n!` of the whole part of `x` (FL: "scalar part"), 0 ≤ n ≤ 170; beyond, infinity.
fn factorial(x: f64) -> f64 {
    if !x.is_finite() || x < 0.0 {
        return f64::NAN;
    }
    let n = x.trunc();
    if n > 170.0 {
        return f64::INFINITY;
    }
    let mut p = 1.0f64;
    for k in 2..=(n as u32) {
        p *= f64::from(k);
    }
    p
}

/// Delphi's integer division and remainder on the whole parts; by zero, 0.
fn int_div(a: f64, b: f64) -> f64 {
    let (a, b) = (a.trunc(), b.trunc());
    if b == 0.0 {
        0.0
    } else {
        (a / b).trunc()
    }
}

fn int_mod(a: f64, b: f64) -> f64 {
    let (a, b) = (a.trunc(), b.trunc());
    if b == 0.0 {
        0.0
    } else {
        a - b * (a / b).trunc()
    }
}

pub(super) fn binary(op: Op, a: f64, b: f64) -> f64 {
    match op {
        Op::Add => a + b,
        Op::Sub => a - b,
        Op::Mul => a * b,
        Op::Div => a / b,
        Op::Pow => math::powf(a, b),
        Op::IDiv => int_div(a, b),
        Op::Mod => int_mod(a, b),
        Op::FMod => {
            if b == 0.0 {
                0.0
            } else {
                a - b * (a / b).trunc()
            }
        }
        Op::Shl => a * math::exp2(b),
        Op::Shr => a / math::exp2(b),
        Op::And => bit(truth(a) && truth(b)),
        Op::Or => bit(truth(a) || truth(b)),
        Op::Xor => bit(truth(a) != truth(b)),
        Op::Eq => bit(a == b),
        Op::Ne => bit(a != b),
        Op::Lt => bit(a < b),
        Op::Le => bit(a <= b),
        Op::Gt => bit(a > b),
        Op::Ge => bit(a >= b),
        _ => f64::NAN,
    }
}

/// Fold `x` into 0…1 by reflection (a wave folder).
fn fold(x: f64) -> f64 {
    let m = x.rem_euclid(2.0);
    if m > 1.0 {
        2.0 - m
    } else {
        m
    }
}

pub(super) fn call(f: Func, a: &[f64]) -> f64 {
    use Func::*;
    let x = a[0];
    match f {
        Sin => math::sin(x),
        Cos => math::cos(x),
        Tan => math::tan(x),
        Sec => 1.0 / math::cos(x),
        CoSec => 1.0 / math::sin(x),
        Ctg => 1.0 / math::tan(x),
        ArcSin => math::asin(x),
        ArcCos => math::acos(x),
        ArcTan => math::atan(x),
        Sinh => math::sinh(x),
        Cosh => math::cosh(x),
        Tanh => math::tanh(x),
        ArcCosh => math::ln(x + (x * x - 1.0).sqrt()),
        ArcTanh => 0.5 * math::ln((1.0 + x) / (1.0 - x)),
        DegToRad => x * (PI / 180.0),
        RadToDeg => x * (180.0 / PI),
        Int => x.trunc(),
        Floor => x.floor(),
        Ceil => x.ceil(),
        Round => x.round_ties_even(),
        Frac => frac(x),
        Sqrt => x.sqrt(),
        Sqr => x * x,
        Exp => math::exp(x),
        Ln => math::ln(x),
        Log10 => math::log10(x),
        Log2 => math::log2(x),
        Abs => x.abs(),
        Neg => -x,
        Fold => fold(x),
        SeededRand => {
            let n = x.trunc() as i64;
            0.5 * (f64::from(cantus_dsp::modfx_clock::random_bipolar(0x5EED, n)) + 1.0)
        }
        Sine => 0.5 - 0.5 * math::cos(2.0 * PI * cycle(x)),
        Saw => cycle(x),
        Tri => 1.0 - (2.0 * cycle(x) - 1.0).abs(),
        Square => bit(cycle(x) < 0.5),
        ATan2 => math::atan2(x, a[1]),
        // Delphi's LogN(Base, X).
        LogN => math::ln(a[1]) / math::ln(x),
        Power => math::powf(x, a[1]),
        IntPower => {
            let p = a[1].trunc();
            if p.abs() > 4096.0 {
                math::powf(x, p)
            } else {
                math::powi(x, p as i32)
            }
        }
        Max => x.max(a[1]),
        Min => x.min(a[1]),
        Sum => x + a[1],
        IfE => bit(x == a[1]),
        IfG => bit(x > a[1]),
        IfGE => bit(x >= a[1]),
        IfL => bit(x < a[1]),
        IfLE => bit(x <= a[1]),
        Pulse => bit(cycle(x) < a[1]),
        // FL: b when a is 1, else c.
        Case => {
            if x == 1.0 {
                a[1]
            } else {
                a[2]
            }
        }
        If => {
            if truth(x) {
                a[1]
            } else {
                a[2]
            }
        }
        Inter => {
            let (lo, hi) = if a[1] <= a[2] { (a[1], a[2]) } else { (a[2], a[1]) };
            bit(lo <= x && x <= hi)
        }
        InterNoSwap => bit(a[1] <= x && x <= a[2]),
        Map => a[1] + x * (a[2] - a[1]),
        Clamp => {
            let (lo, hi) = if a[1] <= a[2] { (a[1], a[2]) } else { (a[2], a[1]) };
            if x.is_nan() {
                x
            } else {
                x.clamp(lo, hi)
            }
        }
    }
}
