//! `powf` for `f32`, fast and correctly rounded in all but the rarest halfway cases, and
//! `db_to_gain` (`DSP_VERSION` 3, round 22).
//!
//! Round 21's [`libm::powf`] (musl's) costs 80–160 ns a call, four to eight times the
//! UCRT's, and the units that call it per sample — Porta's smoothed gain, Sibilus's
//! reduction, Terminus's (and so Culmen's) gain stages — paid for it. An `f32` power has
//! a cheap route: `x^y = 2^(y · log2 x)`, both halves in `f64`. The `f64` intermediate
//! carries about 44 good bits, 20 more than the `f32` result needs, so the only error that
//! matters is the final rounding to `f32`: ≤ 0.5 ulp plus at most 2⁻¹⁴ ulp, which in
//! 1.4 million test points never once picked another `f32` than `pow` in `f64` rounded
//! (`docs/round22/math2.md` has the ulp and ns measurements).
//!
//! Both halves are our own short kernels in the shape of ARM optimized-routines' `powf`
//! (one table step, then a short polynomial; no division), carried to more terms because
//! the target here is correct rounding, not 0.82 ulp. Their tables and coefficients are
//! computed by the compiler's `const` evaluation — IEEE arithmetic, the same bits whatever
//! machine builds them — from the Taylor series' exact rationals. At run time they are
//! plain `f64` adds and multiplies in a fixed order, which Rust never contracts into FMAs:
//! deterministic on every machine, like the rest of [`crate::math`].
//!
//! Everything that is not `finite x > 0` with `finite y` (zeros, negative bases, infinities,
//! NaNs) goes to [`libm::powf`] unchanged, so IEEE 754's special cases (`pow(x, ±0) = 1`,
//! `pow(-8, 1/3) = NaN`, `pow(-2, 3) = -8`, …) are exactly round 21's.

use std::f64::consts::LN_2;

/// `1 / ln 2`, folded by the compiler.
const INV_LN2: f64 = 1.0 / LN_2;

/// `ln(2)^k / k!`: `2^f = Σ B[k] · f^k`. Six terms: with `|f| ≤ 1/64` the first left out
/// is 2⁻⁴⁸ of the result.
const B: [f64; 6] = {
    let mut e = [1.0f64; 6];
    let mut k = 1;
    while k < 6 {
        e[k] = e[k - 1] * LN_2 / k as f64;
        k += 1;
    }
    e
};

/// `1.5 · 2⁵²`: adding it rounds an `f64` of magnitude under 2⁵¹ to an integer, ties to
/// even, leaving the integer in the low bits.
const SHIFT: f64 = 6_755_399_441_055_744.0;

/// `log2(v)` for `v` in `[½, 2]`, to an ulp or two, for building tables at compile time
/// (thirty terms of the `atanh` series; `const` evaluation is IEEE arithmetic, so the tables
/// are the same bits whatever machine builds them).
const fn const_log2(v: f64) -> f64 {
    let s = (v - 1.0) / (v + 1.0);
    let s2 = s * s;
    let (mut term, mut sum, mut k) = (s, 0.0, 0);
    while k < 30 {
        sum += term / (2 * k + 1) as f64;
        term *= s2;
        k += 1;
    }
    2.0 * sum / LN_2
}

/// `2^v` for `v` in `[0, 1)`, to an ulp or two, at compile time (thirty Taylor terms).
const fn const_exp2(v: f64) -> f64 {
    let x = v * LN_2;
    let (mut term, mut sum, mut k) = (1.0, 0.0, 1);
    while k < 31 {
        sum += term;
        term = term * x / k as f64;
        k += 1;
    }
    sum
}

/// The reduction's offset (the bits of ≈ 0.699, ARM optimized-routines' choice): `x` is
/// split as `2^k · z` with `z` in `[OFF, 2·OFF)`, which puts 1 in the middle of an interval.
const OFF: u32 = 0x3f33_0000;
/// Intervals of `z`, by the top six bits of its mantissa (above `OFF`).
const LOG_N: usize = 64;

/// Per interval: `1/c` for its centre `c`, cut to 29 significant bits so that `z · (1/c)`
/// (24 bits × 29) is exact in `f64`, and `−log2` of it. The interval around 1 gets exactly
/// `(1, 0)`, so `log2` stays relatively accurate where it vanishes.
const LOG_T: [(f64, f64); LOG_N] = {
    let mut t = [(0.0, 0.0); LOG_N];
    let mut i = 0;
    while i < LOG_N {
        let centre = f32::from_bits(OFF + ((i as u32) << 17) + (1 << 16)) as f64;
        let invc = f64::from_bits((1.0 / centre).to_bits() & !((1u64 << 24) - 1));
        t[i] = (invc, -const_log2(invc));
        i += 1;
    }
    t
};

/// `log2(1 + r) = Σ A[k] · r^(k+1)`, `A[k] = (−1)^k / ((k+1) · ln 2)`. Six terms: with
/// `|r| < 2⁻⁷` the first left out is under 2⁻⁴⁴ of the sum.
const A: [f64; 6] = [
    INV_LN2,
    -INV_LN2 / 2.0,
    INV_LN2 / 3.0,
    -INV_LN2 / 4.0,
    INV_LN2 / 5.0,
    -INV_LN2 / 6.0,
];

/// `2^(j/32)`.
const EXP_N: usize = 32;
const EXP_T: [f64; EXP_N] = {
    let mut t = [0.0; EXP_N];
    let mut j = 0;
    while j < EXP_N {
        t[j] = const_exp2(j as f64 / EXP_N as f64);
        j += 1;
    }
    t
};

/// `log2(x)` for `x` a finite, positive `f32` (subnormals included), to about 2⁻⁴⁴
/// relative: one table step and a degree-6 polynomial, no division.
#[inline]
fn log2_kernel(x: f32) -> f64 {
    let mut ix = x.to_bits();
    if ix < 0x0080_0000 {
        // Subnormal: normalise, and take the 23 back from the exponent below.
        ix = (x * 8_388_608.0).to_bits().wrapping_sub(23 << 23);
    }
    let tmp = ix.wrapping_sub(OFF);
    let i = ((tmp >> 17) as usize) % LOG_N;
    let k = (tmp as i32) >> 23;
    let z = f32::from_bits(ix.wrapping_sub(tmp & 0xff80_0000)) as f64;
    let (invc, logc) = LOG_T[i];
    // Exact: the product has at most 53 significant bits, and it is within 2⁻⁷ of 1.
    let r = z * invc - 1.0;
    let r2 = r * r;
    let q = (A[0] + A[1] * r) + r2 * (A[2] + A[3] * r) + (r2 * r2) * (A[4] + A[5] * r);
    (k as f64 + logc) + r * q
}

/// `2^t` for `t` in `(−150, 128)`, to about 2⁻⁴⁷ relative: one table step and a degree-5
/// polynomial. Never subnormal in `f64`.
#[inline]
fn exp2_kernel(t: f64) -> f64 {
    // t = n/32 + f, |f| ≤ 1/64; t·32 is exact, and so is f.
    let kd = t * EXP_N as f64 + SHIFT;
    let n = kd.to_bits() as i64 as i32;
    let f = t - (kd - SHIFT) / EXP_N as f64;
    // 2^(n/32) = 2^(j/32) · 2^m: the table entry with m added to its exponent (normal for
    // every n here).
    let (j, m) = ((n & (EXP_N as i32 - 1)) as usize, n >> 5);
    let scale = f64::from_bits(EXP_T[j].to_bits().wrapping_add(((m as i64) << 52) as u64));
    let f2 = f * f;
    let p = (B[0] + B[1] * f) + f2 * (B[2] + B[3] * f) + (f2 * f2) * (B[4] + B[5] * f);
    scale * p
}

/// `2^t` rounded to `f32`, for any finite `t`.
#[inline]
fn to_f32(t: f64) -> f32 {
    if t >= 128.0 {
        // ≥ 2¹²⁸: past f32::MAX by more than half an ulp.
        f32::INFINITY
    } else if t <= -150.0 {
        // ≤ 2⁻¹⁵⁰: half the smallest subnormal or less, which rounds (to even) to 0.
        0.0
    } else {
        exp2_kernel(t) as f32
    }
}

/// `x^y` in `f32`: the kernels above for finite `x > 0` and finite `y`, libm's `powf` for
/// every other input.
#[inline]
pub(super) fn powf(x: f32, y: f32) -> f32 {
    // `x > 0 && x < ∞` is false for NaN as well.
    if x > 0.0 && x < f32::INFINITY && y.is_finite() {
        to_f32(y as f64 * log2_kernel(x))
    } else {
        libm::powf(x, y)
    }
}

/// `log2(10) / 20`: decibels to the exponent of 2.
const DB_TO_LOG2: f64 = std::f64::consts::LOG2_10 / 20.0;

/// `10^(db/20)`: half of [`powf`]'s work (the logarithm is a constant), and the exponent
/// `db/20` is never rounded to `f32` on the way. For a finite `db`; `+∞` gives `+∞`, `−∞`
/// gives 0 and NaN gives NaN, as `powf(10, db / 20)` would.
#[inline]
pub(super) fn db_to_gain(db: f32) -> f32 {
    if db.is_finite() {
        to_f32(db as f64 * DB_TO_LOG2)
    } else {
        libm::powf(10.0, db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// xorshift64*: the tests' own seeded points.
    struct Rng(u64);
    impl Rng {
        fn u(&mut self) -> f64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
        }
        fn uni(&mut self, lo: f64, hi: f64) -> f32 {
            (lo + (hi - lo) * self.u()) as f32
        }
        fn logu(&mut self, lo: f64, hi: f64) -> f32 {
            let (a, b) = (libm::log(lo), libm::log(hi));
            libm::exp(a + (b - a) * self.u()) as f32
        }
    }

    /// The error of `v` in ulps of the `f32` result, against `pow` in `f64` (itself within
    /// an `f64` ulp, 2⁻²⁹ of an `f32` one).
    fn ulps(v: f32, x: f32, y: f32) -> f64 {
        let r = libm::pow(x as f64, y as f64);
        if v == r as f32 {
            return 0.0;
        }
        if !v.is_finite() || !(r as f32).is_finite() {
            return f64::INFINITY;
        }
        let e = ((r as f32).abs().to_bits() >> 23) as i32;
        let ulp = libm::exp2(if e == 0 {
            -149.0
        } else {
            (e - 127 - 23) as f64
        });
        (v as f64 - r).abs() / ulp
    }

    /// Correctly rounded except where the true value lies within 2⁻¹⁰ ulp of a halfway
    /// point, over the ranges the units use and well beyond them.
    #[test]
    fn powf_is_within_half_an_ulp() {
        let mut rng = Rng(0x2205_0f32);
        type Draw = fn(&mut Rng) -> (f32, f32);
        let cases: [(&str, Draw); 7] = [
            ("10^[-7.5,2]", |r| (10.0, r.uni(-7.5, 2.0))),
            ("2^[-10,10]", |r| (2.0, r.uni(-10.0, 10.0))),
            ("(0,1]^[0.1,10]", |r| (r.uni(1e-6, 1.0), r.uni(0.1, 10.0))),
            ("[1e-38,1e38]^[-4,4]", |r| {
                (r.logu(1e-38, 1e38), r.uni(-4.0, 4.0))
            }),
            ("[0.999,1.001]^[-1e4,1e4]", |r| {
                (r.uni(0.999, 1.001), r.uni(-1e4, 1e4))
            }),
            ("subnormal^[-1,1]", |r| {
                (r.logu(1.5e-45, 1e-38), r.uni(-1.0, 1.0))
            }),
            ("edges: 2^(127..128), 2^(-151..-148)", |r| {
                let x = r.logu(1.1, 1e4);
                let t = if r.u() < 0.5 {
                    r.uni(127.0, 128.2)
                } else {
                    r.uni(-151.0, -148.0)
                };
                (x, (t as f64 / libm::log2(x as f64)) as f32)
            }),
        ];
        for (name, draw) in cases {
            let (mut worst, mut wrong) = (0f64, 0usize);
            for _ in 0..200_000 {
                let (x, y) = draw(&mut rng);
                let e = ulps(powf(x, y), x, y);
                assert!(e < 0.5 + 1.0 / 1024.0, "{name}: {x:e}^{y:e} off by {e} ulp");
                worst = worst.max(e);
                wrong += (e > 0.0) as usize;
            }
            eprintln!("{name}: worst {worst:.6} ulp, {wrong} not the nearest f32's bits");
        }
    }

    /// The kernels to the accuracy their comments promise, over every `f32` binade and
    /// bit by bit around 1 (where `log2` vanishes and only a relative error will do).
    #[test]
    fn the_kernels_hold_their_bounds() {
        let mut rng = Rng(0x10_92);
        let rel = |v: f64, r: f64| {
            if r == 0.0 {
                v.abs()
            } else {
                (v - r).abs() / r.abs()
            }
        };
        let (mut log2_worst, mut exp2_worst) = (0f64, 0f64);
        let mut log2_at = |x: f32| {
            log2_worst = log2_worst.max(rel(log2_kernel(x), libm::log2(x as f64)));
        };
        for _ in 0..500_000 {
            log2_at(rng.logu(1.4e-45, 3.4e38));
        }
        for i in 0..200_000u32 {
            log2_at(f32::from_bits(1f32.to_bits() + i));
            log2_at(f32::from_bits(1f32.to_bits() - i));
        }
        for _ in 0..500_000 {
            let t = -150.0 + 278.0 * rng.u();
            exp2_worst = exp2_worst.max(rel(exp2_kernel(t), libm::exp2(t)));
        }
        eprintln!(
            "relative error: log2 2^{:.1}, exp2 2^{:.1}",
            libm::log2(log2_worst),
            libm::log2(exp2_worst),
        );
        assert!(
            log2_worst < libm::exp2(-44.0),
            "log2 kernel: {log2_worst:e}"
        );
        assert!(
            exp2_worst < libm::exp2(-47.0),
            "exp2 kernel: {exp2_worst:e}"
        );
    }

    /// `db_to_gain` is `10^(db/20)` correctly rounded (against `f64`), over every level a
    /// unit uses and to both ends of `f32`, and its special values are `powf`'s.
    #[test]
    fn db_to_gain_is_within_half_an_ulp() {
        let mut rng = Rng(0xdb);
        let mut wrong = 0;
        for i in 0..400_000 {
            let db = if i % 2 == 0 {
                rng.uni(-200.0, 40.0)
            } else {
                rng.uni(-3_100.0, 800.0)
            };
            let r = libm::pow(10.0, db as f64 / 20.0);
            let v = db_to_gain(db);
            if v != r as f32 {
                wrong += 1;
                let ulp =
                    libm::exp2(((r as f32).to_bits() >> 23) as f64 - 150.0).max(libm::exp2(-149.0));
                assert!(
                    (v as f64 - r).abs() / ulp < 0.5 + 1.0 / 1024.0,
                    "{db} dB: {v:e} against {r:e}"
                );
            }
        }
        eprintln!("db_to_gain: {wrong} of 400000 not the nearest f32's bits");
        assert_eq!(db_to_gain(0.0), 1.0);
        assert_eq!(db_to_gain(-0.0), 1.0);
        assert_eq!(db_to_gain(20.0), 10.0);
        assert_eq!(db_to_gain(-40.0), 0.01);
        assert_eq!(db_to_gain(f32::INFINITY), f32::INFINITY);
        assert_eq!(db_to_gain(f32::NEG_INFINITY), 0.0);
        assert!(db_to_gain(f32::NAN).is_nan());
    }

    /// What IEEE 754 defines exactly, exact; everything off the fast path is libm's.
    #[test]
    fn exact_cases_and_special_values() {
        for n in -149..=127 {
            assert_eq!(powf(2.0, n as f32), libm::exp2(n as f64) as f32, "2^{n}");
        }
        assert_eq!(powf(2.0, 128.0), f32::INFINITY);
        assert_eq!(powf(2.0, -150.0), 0.0);
        assert_eq!(powf(0.5, 149.0), f32::from_bits(1));
        assert_eq!(powf(10.0, 39.0), f32::INFINITY);
        for (x, y, want) in [
            (10.0f32, 2.0f32, 100.0f32),
            (10.0, -1.0, 0.1),
            (10.0, 0.0, 1.0),
            (10.0, -0.0, 1.0),
            (9.0, 0.5, 3.0),
            (27.0, 1.0 / 3.0, 3.0),
            (1.0, 1e30, 1.0),
            (1.0, -7.25, 1.0),
            (3.0, 4.0, 81.0),
            (0.25, -2.0, 16.0),
            (f32::MAX, 1.0, f32::MAX),
            (f32::MIN_POSITIVE, 1.0, f32::MIN_POSITIVE),
            (f32::from_bits(1), 1.0, f32::from_bits(1)),
        ] {
            assert_eq!(powf(x, y), want, "{x}^{y}");
        }
        let mut rng = Rng(7);
        for _ in 0..10_000 {
            let x = rng.logu(1e-30, 1e30);
            assert_eq!(powf(x, 1.0), x);
        }
        let specials = [
            0.0f32,
            -0.0,
            1.0,
            -1.0,
            -2.0,
            -8.0,
            0.5,
            -0.5,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            f32::MAX,
            -f32::MAX,
            f32::from_bits(1),
            3.0,
        ];
        for &x in &specials {
            for &y in &specials {
                let (a, b) = (powf(x, y), libm::powf(x, y));
                let fast = x > 0.0 && x.is_finite() && y.is_finite();
                if !fast {
                    assert_eq!(a.to_bits(), b.to_bits(), "{x}^{y}: {a} against libm's {b}");
                } else {
                    assert_eq!(a, b, "{x}^{y}: {a} against libm's {b}");
                }
            }
        }
    }
}
