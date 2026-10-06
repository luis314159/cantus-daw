"""Reference values for `math_report` (round 21, `docs/round21/math.md`).

For every function, type and input range the units use, draws seeded inputs and writes
the true result at 200 bits as a double-double (hi + lo), so the report can measure each
implementation's error in ulps. Needs Python 3 and mpmath:

    python crates/cantus-dsp/examples/math_reference.py target/math_ref.txt
    cargo run -p cantus-dsp --release --example math_report -- target/math_ref.txt

One line per point: `case f32|f64 x y hi lo`, the floats as hex bits of f64 (an f32 input
is exact in f64; `y` is 0 for one-argument functions).
"""
import random
import struct
import sys

import mpmath
from mpmath import mp, mpf

mp.prec = 200
N = 4000
TAU = 6.283185307179586


def bits(v):
    return struct.unpack("<Q", struct.pack("<d", v))[0]


def as_f32(v):
    return struct.unpack("<f", struct.pack("<f", v))[0]


def uni(lo, hi):
    return lambda r: r.uniform(lo, hi)


def logu(lo, hi):
    import math
    a, b = math.log(lo), math.log(hi)
    return lambda r: math.exp(r.uniform(a, b))


def const(c):
    return lambda r: c


# (case name, function, x sampler, y sampler or None) — the ranges are the call sites':
# phases and angular frequencies, prewarped cutoffs, time constants, semitones and
# octaves, levels in dB, curve exponents, saturator drives, pan angles.
CASES = [
    ("sin [0,2pi]", mpmath.sin, uni(0, TAU), None),
    ("sin [0,1e4]", mpmath.sin, uni(0, 1e4), None),
    ("cos [0,2pi]", mpmath.cos, uni(0, TAU), None),
    ("tan [0,pi/2)", mpmath.tan, uni(0, 1.5707), None),
    ("exp [-30,3]", mpmath.exp, uni(-30, 3), None),
    ("exp2 [-24,24]", lambda x: mpf(2) ** x, uni(-24, 24), None),
    ("ln [1e-6,1e4]", mpmath.log, logu(1e-6, 1e4), None),
    ("log2 [1e-6,1e4]", lambda x: mpmath.log(x, 2), logu(1e-6, 1e4), None),
    ("log10 [1e-10,10]", mpmath.log10, logu(1e-10, 10), None),
    ("tanh [-10,10]", mpmath.tanh, uni(-10, 10), None),
    ("atan [-10,10]", mpmath.atan, uni(-10, 10), None),
    ("sinh [-5,5]", mpmath.sinh, uni(-5, 5), None),
    ("ln_1p [-0.5,10]", lambda x: mpmath.log(1 + x), uni(-0.5, 10), None),
    ("cbrt [1e-6,1e3]", mpmath.cbrt, logu(1e-6, 1e3), None),
    ("powf 10^[-7.5,2]", lambda x, y: x ** y, const(10.0), uni(-7.5, 2)),
    ("powf 2^[-10,10]", lambda x, y: x ** y, const(2.0), uni(-10, 10)),
    ("powf (0,1]^[0.1,10]", lambda x, y: x ** y, uni(1e-6, 1), uni(0.1, 10)),
    ("atan2 [-1,1]^2", lambda y, x: mpmath.atan2(y, x), uni(-1, 1), uni(-1, 1)),
    ("hypot [-1e3,1e3]^2", lambda x, y: mpmath.sqrt(x * x + y * y), uni(-1e3, 1e3), uni(-1e3, 1e3)),
]


def main():
    out = open(sys.argv[1] if len(sys.argv) > 1 else "math_ref.txt", "w")
    rng = random.Random(21)
    for name, f, xs, ys in CASES:
        for ty in ("f32", "f64"):
            for _ in range(N):
                x = xs(rng)
                y = ys(rng) if ys else 0.0
                if ty == "f32":
                    x, y = as_f32(x), as_f32(y)
                v = f(mpf(x), mpf(y)) if ys else f(mpf(x))
                hi = float(v)
                lo = float(v - mpf(hi))
                out.write(f"{name}|{ty}|{bits(x):x}|{bits(y):x}|{bits(hi):x}|{bits(lo):x}\n")
    out.close()


if __name__ == "__main__":
    main()
