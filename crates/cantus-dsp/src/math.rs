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

// STEP 1 BACKEND (std) — proves the call-site rewrite changes no bit; replaced by libm in
// the DSP_VERSION commit.
#[allow(clippy::disallowed_methods)]
mod backend {
    use super::Float;
    float_impl!(f32,
        unary: {
            m_sin => f32::sin, m_cos => f32::cos, m_tan => f32::tan,
            m_asin => f32::asin, m_acos => f32::acos, m_atan => f32::atan,
            m_sinh => f32::sinh, m_cosh => f32::cosh, m_tanh => f32::tanh,
            m_exp => f32::exp, m_exp2 => f32::exp2, m_exp_m1 => f32::exp_m1,
            m_ln => f32::ln, m_log2 => f32::log2, m_log10 => f32::log10,
            m_ln_1p => f32::ln_1p, m_cbrt => f32::cbrt,
        },
        binary: { m_atan2 => f32::atan2, m_powf => f32::powf, m_hypot => f32::hypot },
        sin_cos: f32::sin_cos,
    );
    float_impl!(f64,
        unary: {
            m_sin => f64::sin, m_cos => f64::cos, m_tan => f64::tan,
            m_asin => f64::asin, m_acos => f64::acos, m_atan => f64::atan,
            m_sinh => f64::sinh, m_cosh => f64::cosh, m_tanh => f64::tanh,
            m_exp => f64::exp, m_exp2 => f64::exp2, m_exp_m1 => f64::exp_m1,
            m_ln => f64::ln, m_log2 => f64::log2, m_log10 => f64::log10,
            m_ln_1p => f64::ln_1p, m_cbrt => f64::cbrt,
        },
        binary: { m_atan2 => f64::atan2, m_powf => f64::powf, m_hypot => f64::hypot },
        sin_cos: f64::sin_cos,
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
