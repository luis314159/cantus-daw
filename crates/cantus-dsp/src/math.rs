//! The transcendental functions every Cantus unit calls: the same bits on every machine.
//!
//! **Why not std's.** `f32::sin`, `f64::exp`, `powf` and the rest call the platform's C
//! library, whose precision Rust leaves "unspecified … varies by platform". On Windows the
//! UCRT goes further and picks an FMA3 or a non-FMA3 implementation of `exp`, `sin`, `pow`
//! … by CPU at start-up, documenting that results may differ "between computers that do or
//! don't support FMA3". So the same Cantus binary rendered different bits on a pre-Haswell
//! CPU, on macOS (Apple's libm) and on Linux (glibc) — and a render cache, a published
//! golden or a null test across machines cannot live with that (ADR 0031 R9).
//!
//! **What instead.** Every function here is the pinned pure-Rust [`libm`] crate (`=0.2.16`,
//! a port of musl and of ARM's optimized-routines): ordinary Rust arithmetic in a fixed
//! order, which Rust never contracts into fused multiply-adds, so it rounds the same way on
//! every x86-64 and aarch64 machine. Its only CPU-dependent code is `sqrt` and `fma`
//! (hardware or software), both correctly rounded and so identical either way; only
//! [`cbrt`] uses `fma`. The choice against std and against polynomials of our own is
//! measured in `docs/round21/math.md` (accuracy in ulps over the ranges the units use,
//! and speed).
//!
//! [`powi`] is our own: the repeated squaring of compiler-rt's `__powidf2`, written out so
//! that neither LLVM's expansion of a constant exponent nor a library decides the order.
//!
//! **The rule.** Inside `cantus-dsp` (and the engine's DSP paths) std's transcendental
//! methods are banned by `clippy::disallowed_methods` (`clippy.toml`); call these instead:
//! `math::sin(x)` for `x.sin()`, `math::powf(x, y)` for `x.powf(y)`. Changing what any of
//! them returns changes rendered bits: it is a [`crate::DSP_VERSION`] bump, with every
//! golden re-blessed in the same commit.
//!
//! `sqrt`, `abs`, `floor`, `mul_add` and the like stay std's: IEEE 754 requires them
//! correctly rounded, so they are the same everywhere. Approximate reciprocal and
//! reciprocal-square-root instructions (`rcpps`, `rsqrtps`, which differ between Intel and
//! AMD) are never used.

mod sealed {
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

/// `f32` or `f64`: the two types the functions below accept.
pub trait Float: Copy + sealed::Sealed {
    #[doc(hidden)]
    fn m_sin(self) -> Self;
    #[doc(hidden)]
    fn m_cos(self) -> Self;
    #[doc(hidden)]
    fn m_tan(self) -> Self;
    #[doc(hidden)]
    fn m_sin_cos(self) -> (Self, Self);
    #[doc(hidden)]
    fn m_asin(self) -> Self;
    #[doc(hidden)]
    fn m_acos(self) -> Self;
    #[doc(hidden)]
    fn m_atan(self) -> Self;
    #[doc(hidden)]
    fn m_atan2(self, x: Self) -> Self;
    #[doc(hidden)]
    fn m_sinh(self) -> Self;
    #[doc(hidden)]
    fn m_cosh(self) -> Self;
    #[doc(hidden)]
    fn m_tanh(self) -> Self;
    #[doc(hidden)]
    fn m_exp(self) -> Self;
    #[doc(hidden)]
    fn m_exp2(self) -> Self;
    #[doc(hidden)]
    fn m_exp_m1(self) -> Self;
    #[doc(hidden)]
    fn m_ln(self) -> Self;
    #[doc(hidden)]
    fn m_log2(self) -> Self;
    #[doc(hidden)]
    fn m_log10(self) -> Self;
    #[doc(hidden)]
    fn m_ln_1p(self) -> Self;
    #[doc(hidden)]
    fn m_powf(self, y: Self) -> Self;
    #[doc(hidden)]
    fn m_powi(self, n: i32) -> Self;
    #[doc(hidden)]
    fn m_hypot(self, y: Self) -> Self;
    #[doc(hidden)]
    fn m_cbrt(self) -> Self;
}

/// One implementation of [`Float`] per type: each method one call into the backend, and
/// [`powi`]'s own loop.
macro_rules! float_impl {
    (
        $t:ty,
        unary: { $($name:ident => $f:path),* $(,)? },
        binary: { $($bname:ident => $bf:path),* $(,)? },
        sin_cos: $sc:path $(,)?
    ) => {
        impl Float for $t {
            $(
                #[inline]
                fn $name(self) -> Self {
                    $f(self)
                }
            )*
            $(
                #[inline]
                fn $bname(self, other: Self) -> Self {
                    $bf(self, other)
                }
            )*
            #[inline]
            fn m_sin_cos(self) -> (Self, Self) {
                $sc(self)
            }
            #[inline]
            fn m_powi(self, n: i32) -> Self {
                // compiler-rt's `__powidf2`, step for step: the bits do not depend on
                // whether LLVM expands a constant exponent or calls the runtime.
                let mut a = self;
                let mut b = n;
                let mut r: $t = 1.0;
                loop {
                    if b & 1 != 0 {
                        r *= a;
                    }
                    b /= 2;
                    if b == 0 {
                        break;
                    }
                    a *= a;
                }
                if n < 0 {
                    1.0 / r
                } else {
                    r
                }
            }
        }
    };
}

// The backend: libm, pinned (`=0.2.16` in the workspace's Cargo.toml). Moving the pin is a
// `DSP_VERSION` bump like any other change of bits.
mod backend {
    use super::Float;
    float_impl!(f32,
        unary: {
            m_sin => libm::sinf, m_cos => libm::cosf, m_tan => libm::tanf,
            m_asin => libm::asinf, m_acos => libm::acosf, m_atan => libm::atanf,
            m_sinh => libm::sinhf, m_cosh => libm::coshf, m_tanh => libm::tanhf,
            m_exp => libm::expf, m_exp2 => libm::exp2f, m_exp_m1 => libm::expm1f,
            m_ln => libm::logf, m_log2 => libm::log2f, m_log10 => libm::log10f,
            m_ln_1p => libm::log1pf, m_cbrt => libm::cbrtf,
        },
        binary: { m_atan2 => libm::atan2f, m_powf => libm::powf, m_hypot => libm::hypotf },
        sin_cos: libm::sincosf,
    );
    float_impl!(f64,
        unary: {
            m_sin => libm::sin, m_cos => libm::cos, m_tan => libm::tan,
            m_asin => libm::asin, m_acos => libm::acos, m_atan => libm::atan,
            m_sinh => libm::sinh, m_cosh => libm::cosh, m_tanh => libm::tanh,
            m_exp => libm::exp, m_exp2 => libm::exp2, m_exp_m1 => libm::expm1,
            m_ln => libm::log, m_log2 => libm::log2, m_log10 => libm::log10,
            m_ln_1p => libm::log1p, m_cbrt => libm::cbrt,
        },
        binary: { m_atan2 => libm::atan2, m_powf => libm::pow, m_hypot => libm::hypot },
        sin_cos: libm::sincos,
    );
}

/// The sine of `x` radians.
#[inline]
pub fn sin<T: Float>(x: T) -> T {
    x.m_sin()
}
/// The cosine of `x` radians.
#[inline]
pub fn cos<T: Float>(x: T) -> T {
    x.m_cos()
}
/// The tangent of `x` radians.
#[inline]
pub fn tan<T: Float>(x: T) -> T {
    x.m_tan()
}
/// `(sin x, cos x)` in one call.
#[inline]
pub fn sin_cos<T: Float>(x: T) -> (T, T) {
    x.m_sin_cos()
}
/// The arcsine, in radians.
#[inline]
pub fn asin<T: Float>(x: T) -> T {
    x.m_asin()
}
/// The arccosine, in radians.
#[inline]
pub fn acos<T: Float>(x: T) -> T {
    x.m_acos()
}
/// The arctangent, in radians.
#[inline]
pub fn atan<T: Float>(x: T) -> T {
    x.m_atan()
}
/// The four-quadrant arctangent of `y / x` (std's `y.atan2(x)`).
#[inline]
pub fn atan2<T: Float>(y: T, x: T) -> T {
    y.m_atan2(x)
}
/// The hyperbolic sine.
#[inline]
pub fn sinh<T: Float>(x: T) -> T {
    x.m_sinh()
}
/// The hyperbolic cosine.
#[inline]
pub fn cosh<T: Float>(x: T) -> T {
    x.m_cosh()
}
/// The hyperbolic tangent.
#[inline]
pub fn tanh<T: Float>(x: T) -> T {
    x.m_tanh()
}
/// `e^x`.
#[inline]
pub fn exp<T: Float>(x: T) -> T {
    x.m_exp()
}
/// `2^x`.
#[inline]
pub fn exp2<T: Float>(x: T) -> T {
    x.m_exp2()
}
/// `e^x − 1`, accurate near 0.
#[inline]
pub fn exp_m1<T: Float>(x: T) -> T {
    x.m_exp_m1()
}
/// The natural logarithm.
#[inline]
pub fn ln<T: Float>(x: T) -> T {
    x.m_ln()
}
/// The base-2 logarithm.
#[inline]
pub fn log2<T: Float>(x: T) -> T {
    x.m_log2()
}
/// The base-10 logarithm.
#[inline]
pub fn log10<T: Float>(x: T) -> T {
    x.m_log10()
}
/// `ln(1 + x)`, accurate near 0.
#[inline]
pub fn ln_1p<T: Float>(x: T) -> T {
    x.m_ln_1p()
}
/// `x^y` (std's `x.powf(y)`).
#[inline]
pub fn powf<T: Float>(x: T, y: T) -> T {
    x.m_powf(y)
}
/// `x^n` by repeated squaring, in compiler-rt's order (std's `x.powi(n)`).
#[inline]
pub fn powi<T: Float>(x: T, n: i32) -> T {
    x.m_powi(n)
}
/// `sqrt(x² + y²)` without undue overflow.
#[inline]
pub fn hypot<T: Float>(x: T, y: T) -> T {
    x.m_hypot(y)
}
/// The cube root.
#[inline]
pub fn cbrt<T: Float>(x: T) -> T {
    x.m_cbrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FNV-1a over the bits of `f` at 4096 inputs spread over `[lo, hi]`.
    fn digest<T: Float + Into<f64>>(lo: f64, hi: f64, f: impl Fn(f64) -> T) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for i in 0..4096 {
            let x = lo + (hi - lo) * i as f64 / 4095.0;
            for b in f(x).into().to_bits().to_le_bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
            }
        }
        h
    }

    /// Every function's bits over the ranges the units use, pinned. The same on every
    /// machine is the point of this module: a CI runner of another class (aarch64, a CPU
    /// without FMA3) that disagrees here has found the bug R9 exists to prevent; a change on
    /// purpose (a new libm) is a `DSP_VERSION` bump with the new digests.
    #[test]
    fn the_bits_are_the_pinned_ones() {
        let got: Vec<(&str, u64)> = vec![
            ("sin32", digest(-10.0, 10.0, |x| sin(x as f32))),
            ("sin64", digest(-10.0, 10.0, sin)),
            ("cos32", digest(-10.0, 10.0, |x| cos(x as f32))),
            ("cos64", digest(-10.0, 10.0, cos)),
            ("tan32", digest(0.0, 1.57, |x| tan(x as f32))),
            ("tan64", digest(0.0, 1.57, tan)),
            ("sincos64", digest(-4.0, 4.0, |x| sin_cos(x).0 - 3.0 * sin_cos(x).1)),
            ("tanh32", digest(-10.0, 10.0, |x| tanh(x as f32))),
            ("tanh64", digest(-10.0, 10.0, tanh)),
            ("exp32", digest(-30.0, 3.0, |x| exp(x as f32))),
            ("exp64", digest(-30.0, 3.0, exp)),
            ("exp2_32", digest(-24.0, 24.0, |x| exp2(x as f32))),
            ("exp2_64", digest(-24.0, 24.0, exp2)),
            ("ln32", digest(1e-6, 1e4, |x| ln(x as f32))),
            ("ln64", digest(1e-6, 1e4, ln)),
            ("log2_64", digest(1e-6, 1e4, log2)),
            ("log10_32", digest(1e-6, 10.0, |x| log10(x as f32))),
            ("log10_64", digest(1e-6, 10.0, log10)),
            ("pow10_32", digest(-7.5, 2.0, |x| powf(10f32, x as f32))),
            ("pow10_64", digest(-7.5, 2.0, |x| powf(10f64, x))),
            ("powxy64", digest(1e-3, 1.0, |x| powf(x, 0.1 + 9.0 * x))),
            ("atan2_64", digest(-1.0, 1.0, |x| atan2(x, 0.5 - x))),
            ("powi64", digest(-2.0, 2.0, |x| powi(x, 7) + powi(x, -3))),
        ];
        let want = PINNED;
        if std::env::var_os("CANTUS_MATH_PIN").is_some() {
            for (n, h) in &got {
                println!("    (\"{n}\", 0x{h:016x}),");
            }
        }
        assert_eq!(got.len(), want.len());
        for ((n, h), (wn, wh)) in got.iter().zip(want) {
            assert_eq!(n, wn);
            assert_eq!(h, wh, "{n}: not the pinned bits (CANTUS_MATH_PIN=1 prints the new ones)");
        }
    }

    /// `powi` is compiler-rt's order, and exact where the result is.
    #[test]
    fn powi_squares_as_compiler_rt_does() {
        assert_eq!(powi(2f64, -63), 1.0 / 9_223_372_036_854_775_808.0);
        assert_eq!(powi(3f32, 4), 81.0);
        assert_eq!(powi(-2f64, 3), -8.0);
        assert_eq!(powi(5f64, 0), 1.0);
        let x = 1.000_1f64;
        assert_eq!(powi(x, 5), x * ((x * x) * (x * x)));
    }

    /// Recorded with `DSP_VERSION` 2 (libm 0.2.16) on x86-64 Windows.
    const PINNED: &[(&str, u64)] = &[
        ("sin32", 0x6250a4afb2d478a9),
        ("sin64", 0x90e90bb477a4561e),
        ("cos32", 0xa99a326172bf6c55),
        ("cos64", 0x65c0104890945c14),
        ("tan32", 0xdb97865cabd3b7d3),
        ("tan64", 0xed7c2bfe4e157e43),
        ("sincos64", 0x5cf986ab1c18dce4),
        ("tanh32", 0xa6c8e461d6f706f1),
        ("tanh64", 0xa83234db20b48326),
        ("exp32", 0x3f24a591e8d68b5c),
        ("exp64", 0xed5277a3941e6fc1),
        ("exp2_32", 0x259fe0650bb8c202),
        ("exp2_64", 0x07c7c3bb6d89c1c4),
        ("ln32", 0x8197d7b0c5290a28),
        ("ln64", 0xd75e832eaab48d85),
        ("log2_64", 0xf04fed6bc80a79f6),
        ("log10_32", 0xd2e2b76256cbf0b6),
        ("log10_64", 0x4d3ea5c41ee86928),
        ("pow10_32", 0xb6996d5e6b36f65c),
        ("pow10_64", 0x9cf45f9dd51436d5),
        ("powxy64", 0xed98779dfe93816e),
        ("atan2_64", 0x492dc53a9cc6ec87),
        ("powi64", 0x73133c922cbc7715),
    ];
}
