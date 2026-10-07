# Round 21 — *math*: the same bits on every machine (R9), and dither that comes to rest (R10)

Branch `r21/math`, base 49f02f3. ADR 0031 amendments R9, R10 and R16; `docs/perf/plan.md`
Phase 3 *math* and *dither*.

## The problem

`cantus-dsp` called std's transcendental methods (`sin`, `exp`, `powf`, `log10`, `tanh`…)
at about 600 sites, the engine and `cantus-core` (gains, pan laws, fades, automation curves)
at about 40 more. std defers them to the platform's C library, whose precision Rust leaves
unspecified; on Windows the UCRT picks an FMA3 or a non-FMA3 implementation **by CPU at
start-up**. So one Cantus binary rendered different bits on a pre-Haswell machine, on macOS
and on Linux — and the null tests, the goldens, a shared render cache and the future
cross-machine CI could not hold. LLVM also constant-folds std's calls with the *build*
machine's libm, a third source of difference.

## What was done

1. **`crates/cantus-dsp/src/math.rs`** — one module, free functions over `f32` and `f64`
   (`math::sin(x)` for `x.sin()`, `math::powf(x, y)` for `x.powf(y)`): `sin cos tan sin_cos
   asin acos atan atan2 sinh cosh tanh exp exp2 exp_m1 ln log2 log10 ln_1p powf powi hypot
   cbrt`. Backend: the pinned pure-Rust **`libm = "=0.2.16"`** (musl and ARM
   optimized-routines). `powi` is our own loop, compiler-rt's `__powidf2` step for step, so
   neither LLVM's expansion of a constant exponent nor a library picks the order.
   `va_math.rs` (the VA family's own Padé/Taylor `tanh`, `tan_pi`, `exp2`, `sin_turns`)
   stays: it was already deterministic.
2. **Every call site routed**, by a mechanical rewrite (receiver → first argument), in two
   commits so the rewrite itself is proven: first with a **std backend** — every test and
   every golden hash unchanged, i.e. the rewrite moved no bit — then the backend switched to
   libm alone, which is the only change of bits.
3. **`DSP_VERSION`** in `cantus-dsp/src/lib.rs`, introduced at 1 (std), bumped once to 2
   (libm) in the commit that re-blesses every golden, with a table of versions in its doc.
4. **Lints (R16).** `clippy.toml` in `cantus-dsp`, `cantus-core`, `cantus-engine` and
   `cantus-plugins` bans std's transcendentals for `f32` and `f64` (`sin cos tan sin_cos asin
   acos atan atan2 sinh cosh tanh asinh acosh atanh exp exp2 exp_m1 ln log log2 log10 ln_1p
   powf powi hypot cbrt`) and the approximate SSE/AVX reciprocals (`_mm_rcp_ps/ss`,
   `_mm_rsqrt_ps/ss`, `_mm256_rcp_ps`, `_mm256_rsqrt_ps`). Each file repeats the workspace's
   settings, since clippy reads only the nearest `clippy.toml`. `sqrt`, `abs`, `floor`,
   `mul_add` stay allowed: IEEE 754 makes them correctly rounded everywhere. No kernel used an
   approximate reciprocal (searched: none).
5. **A cross-machine pin**: `math::tests::the_bits_are_the_pinned_ones` hashes every
   function's bits over the units' ranges; a runner of another class (aarch64, a CPU without
   FMA3) that disagrees has found exactly the bug R9 is about.
6. **Measuring tools kept in the tree**: `examples/math_reference.py` (mpmath at 200 bits →
   double-double references) and `examples/math_report.rs` (ulp error and ns/call, std vs
   ours); the golden tests write their samples with `CANTUS_GOLDEN_DUMP=<dir>`, so every
   future bump can state how far each golden moved.

## Why libm (and not our own polynomials, and not std)

*Determinism.* libm is plain Rust arithmetic in a fixed order; Rust never contracts `a*b+c`
into an FMA, so the code rounds the same on every target. Its only CPU-dependent paths on
x86-64 are `sqrt` (`sqrtsd`) and `fma` (hardware or software, chosen at run time), both
correctly rounded and therefore identical; `fma` is used only by `cbrt`. On aarch64 the same
holds (`sqrt`, `fma`, `rint`). Debug and release builds give the same bits (no fast-math).

*Accuracy and speed* — see the tables below.

*Own minimax polynomials* were the alternative. For the full ranges coefficient code uses
(`tan` up to π/2, `exp` over 33 units, `pow` with any base) a polynomial set of libm's
accuracy is a reimplementation of libm, with argument reduction, special cases and a
verification burden we would carry alone — for coefficient code that is not hot. Where a
call *is* per-sample and an approximation is acceptable, Cantus already has its own
(`va_math.rs`, used by the VA filters and oscillators), and R14 (ADAA shapers) is where the
remaining per-sample `tanh` goes next.

## The one change of bits: `DSP_VERSION` 2

Commit `af74c7b` (the backend switch alone; the rewrite before it, `654a67a`, changed no
bit — every test and all four hashes unchanged). Size of the change, max |new − old| over
each golden's f32 render (`CANTUS_GOLDEN_DUMP`, `golden_diff`):

| Golden | Samples | Differing | Max abs diff | dBFS | re the golden's peak |
|---|---:|---:|---:|---:|---:|
| `v5.cantus` (`null_tests` `GOLDEN_V5`) | 1 968 000 | 77.6 % | 2.6·10⁻⁵ | −91.8 | −70.2 dB |
| Pulsus I kit (`GOLDEN_V1_KIT`) | 794 182 | 61.0 % | 3.8·10⁻⁵ | −88.5 | −79.6 dB |
| Pulsus I kit from bar 2 (`GOLDEN_V1_FROM_BAR_2`) | 571 638 | 63.2 % | 3.8·10⁻⁵ | −88.5 | −80.3 dB |
| v14 fixture kit (`GOLDEN_V14_KIT`) | 288 000 | 6.7 % | 3.0·10⁻⁸ | −150.5 | −136.8 dB |

An ulp of difference in a coefficient (a cutoff's `tan`, a decay's `exp`, a pitch's `exp2`)
grows through resonant filters and accumulated phase into differences of about one 16-bit
LSB at most — inaudible, and the reason a bump re-blesses rather than tolerates. No other
test needed any change (every other assertion is a tolerance or a relation, and all held).

## Accuracy and speed, measured

`math_report` (release, pinned to logical CPU 4, a P-core of an i7-12700H, Windows 11 UCRT
with FMA3), 4000 seeded points per case against mpmath at 200 bits. Error in ulps of the
result type; ns per call with an indirect call's cost subtracted; "same bits" is std vs
ours.

| Case | Type | std max | std mean | libm max | libm mean | same bits | std ns | libm ns |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| sin [0,2pi] | f32 | 0.517 | 0.254 | 0.500 | 0.254 | 99.8 % | 41.4 | 25.0 |
| sin [0,2pi] | f64 | 0.745 | 0.252 | 0.745 | 0.252 | 98.1 % | 29.7 | 32.9 |
| sin [0,1e4] | f32 | 0.503 | 0.250 | 0.500 | 0.250 | 100.0 % | 34.2 | 44.1 |
| sin [0,1e4] | f64 | 0.686 | 0.251 | 0.746 | 0.250 | 98.3 % | 28.2 | 48.8 |
| cos [0,2pi] | f32 | 0.515 | 0.251 | 0.500 | 0.251 | 99.7 % | 37.1 | 27.8 |
| cos [0,2pi] | f64 | 0.709 | 0.251 | 0.701 | 0.251 | 97.9 % | 32.1 | 32.6 |
| tan [0,pi/2) | f32 | 0.501 | 0.255 | 0.782 | 0.281 | 85.5 % | 21.3 | 16.3 |
| tan [0,pi/2) | f64 | 0.737 | 0.253 | 0.679 | 0.253 | 95.3 % | 26.9 | 27.2 |
| exp [-30,3] | f32 | 0.500 | 0.250 | 0.789 | 0.265 | 89.8 % | 11.3 | 9.9 |
| exp [-30,3] | f64 | 0.534 | 0.253 | 0.780 | 0.267 | 90.2 % | 6.7 | 13.7 |
| exp2 [-24,24] | f32 | 1.826 | 0.445 | 0.500 | 0.248 | 62.6 % | 112.5 | 8.3 |
| exp2 [-24,24] | f64 | 1.768 | 0.458 | 0.500 | 0.253 | 60.9 % | 129.1 | 7.2 |
| ln [1e-6,1e4] | f32 | 0.517 | 0.249 | 0.701 | 0.251 | 97.0 % | 10.6 | 10.5 |
| ln [1e-6,1e4] | f64 | 0.501 | 0.075 | 0.716 | 0.076 | 99.1 % | 6.7 | 16.9 |
| log2 [1e-6,1e4] | f32 | 1.379 | 0.262 | 0.715 | 0.256 | 97.3 % | 64.8 | 8.9 |
| log2 [1e-6,1e4] | f64 | 0.507 | 0.251 | 0.520 | 0.251 | 99.7 % | 11.3 | 13.5 |
| log10 [1e-10,10] | f32 | 0.508 | 0.251 | 0.772 | 0.253 | 98.0 % | 7.9 | 14.9 |
| log10 [1e-10,10] | f64 | 0.500 | 0.250 | 0.569 | 0.251 | 99.8 % | 6.9 | 13.9 |
| tanh [-10,10] | f32 | 0.818 | 0.242 | 1.727 | 0.255 | 96.9 % | 20.7 | 28.5 |
| tanh [-10,10] | f64 | 0.862 | 0.250 | 1.831 | 0.263 | 97.0 % | 31.3 | 38.6 |
| atan [-10,10] | f32 | 0.500 | 0.249 | 0.714 | 0.256 | 93.4 % | 23.4 | 19.3 |
| atan [-10,10] | f64 | 0.697 | 0.257 | 0.697 | 0.257 | 99.8 % | 24.6 | 18.0 |
| sinh [-5,5] | f32 | 0.500 | 0.250 | 1.502 | 0.344 | 74.5 % | 24.0 | 20.1 |
| sinh [-5,5] | f64 | 0.964 | 0.275 | 1.642 | 0.345 | 72.7 % | 24.3 | 27.2 |
| ln_1p [-0.5,10] | f32 | 0.979 | 0.275 | 0.683 | 0.261 | 87.8 % | 14.5 | 19.5 |
| ln_1p [-0.5,10] | f64 | 0.987 | 0.262 | 0.697 | 0.257 | 90.9 % | 21.4 | 13.9 |
| cbrt [1e-6,1e3] | f32 | 1.402 | 0.378 | 0.500 | 0.248 | 69.8 % | 83.0 | 8.4 |
| cbrt [1e-6,1e3] | f64 | 1.467 | 0.389 | 0.500 | 0.252 | 68.2 % | 111.0 | 21.5 |
| powf 10^[-7.5,2] | f32 | 0.500 | 0.253 | 0.789 | 0.268 | 89.3 % | 15.2 | 80.2 |
| powf 10^[-7.5,2] | f64 | 0.506 | 0.252 | 0.760 | 0.265 | 90.3 % | 38.6 | 87.7 |
| powf 2^[-10,10] | f32 | 0.500 | 0.253 | 0.742 | 0.267 | 89.8 % | 13.9 | 81.8 |
| powf 2^[-10,10] | f64 | 0.505 | 0.250 | 0.801 | 0.262 | 91.0 % | 39.5 | 99.7 |
| powf (0,1]^[0.1,10] | f32 | 0.500 | 0.249 | 0.803 | 0.263 | 89.8 % | 20.0 | 159.6 |
| powf (0,1]^[0.1,10] | f64 | 0.507 | 0.251 | 0.757 | 0.267 | 89.5 % | 63.1 | 312.7 |
| atan2 [-1,1]^2 | f32 | 0.500 | 0.253 | 1.036 | 0.297 | 83.9 % | 66.1 | 65.6 |
| atan2 [-1,1]^2 | f64 | 0.506 | 0.249 | 1.056 | 0.304 | 82.1 % | 66.3 | 72.1 |
| hypot [-1e3,1e3]^2 | f32 | 0.500 | 0.250 | 0.802 | 0.272 | 88.0 % | 9.4 | 9.6 |
| hypot [-1e3,1e3]^2 | f64 | 1.066 | 0.291 | 0.867 | 0.276 | 89.9 % | 14.4 | 12.6 |

**Reading it.** libm is faithfully rounded everywhere the units call it: at most 0.8 ulp,
except `tanh` (1.8), `sinh` (1.6) and `atan2` (1.06) — all far below anything audible, and
the UCRT is no better across the board (its `exp2` 1.8, `log2`/`cbrt` 1.4). The UCRT's FMA3
path is faster at `exp`/`ln`/`log10` (f64, ~2×) and much faster at **`powf`** (f32: 14–20 ns
vs 80–160 ns; f64: 39–63 ns vs 88–313 ns); libm is much faster at `exp2`, `log2` (f32) and
`cbrt` (UCRT's are slow paths), and even at `sin`/`cos` f32.

**On the kernels** (`cantus-bench --quick --only fx,inst`, the std build of `654a67a`
against the libm build of `af74c7b`, alternated twice on the same pinned core): the machine
was shared with six other builders compiling, and round 1 ran 2–6× slower than round 2 for
*both* builds — kernels that call no transcendental per sample (Firmus) moved ±50 %. The
geometric mean over all 51 kernels was 0.95× (no overall loss), but the per-kernel numbers
are not trustworthy; the micro-benchmark above is. Units that call `powf`/`log10` **per
sample** — Porta's smoothed gain (`porta.rs` ~1072) and meters, Sibilus's per-sample gain
(`sibilus.rs` ~496/577), the meters of Culmen and Terminus — will pay libm's `powf` cost.
Re-measure on a quiet machine: `cargo run -p xtask -- perf --only fx,inst --pin 4`.

**The fix, recommended next** (its own reviewed `DSP_VERSION` bump): an `f32` `powf` fast
path, `exp2(y · log2 x)` evaluated in `f64` with libm's fast `exp2`/`log2` for finite
`x > 0` (libm's `powf` for the rest) — ≈ 20 ns and *more* accurate than either (the f64
intermediate leaves only the final rounding); and per-sample `10^(dB/20)` written as
`exp2(dB · log2(10)/20)`. Neither touches determinism.

## Where the rule holds, and where it does not yet

| Crate | Routed | Lint (`clippy.toml`) |
|---|---|---|
| `cantus-dsp` | every unit, its tests, `va_*` (beside `va_math`'s own functions) | yes |
| `cantus-core` | gains and pan laws (`model.rs`), fades (`audio.rs`, `pads.rs`), automation curves and tapers (`automation/`), FM carrier levels — now depends on `cantus-dsp` (a leaf crate) for it | yes |
| `cantus-engine` | everything except `processor*.rs` and `profile.rs` (graph's files this round), and its tests | yes — warns at the 7 sites left, listed in `docs/round21/requests.md` |
| `cantus-plugins` | the exported plug-ins' parameter tapers (`params.rs`), the test signals | yes |
| `cantus-ui`, `cantus-host`, test plug-ins, `cantus-project` | not routed: drawing, hosting third parties, MIDI export — no rendered Cantus DSP | no |

`clippy --all-targets` over the four crates: the only `disallowed_methods` hits are the
graph-owned sites listed in the request (`processor_rec.rs` 319/332, `profile.rs`
51/59/65/371/374).

## Tests

- Step 1 (std backend, `654a67a`): `cargo test -p cantus-dsp -p cantus-core -p cantus-engine
  -p cantus-plugins` — 865 passed, 0 failed; all four golden hashes unchanged.
- Step 2 (libm, `af74c7b`): 863 passed; the 4 failures were exactly the four hash goldens,
  re-blessed in the same commit (then 7/7 and 26/26 in `inst_pulsus`/`null_tests`).
  **`hosted_in_cantus`** — every exported plug-in hosted by Cantus sounds as the native
  insert, to the bit — passes on libm. New: `math::tests::the_bits_are_the_pinned_ones`,
  `powi_squares_as_compiler_rt_does`.
- `cantus-ui`: see the report (run last).

## Left

1. **R10 — counter-based dither (not done this round).** Design, ready to build:
   `FxUnit::process` carries no absolute sample position (adding one is an interface change
   for at-rest and graph), so the count is *samples since the ditherer last came to rest*:
   `Ditherer { seed: u64, n: u64, err, zeros }`, noise for sample `n` = two 24-bit uniforms
   cut from SplitMix64 output `mix64(seed + n·γ)` (`va::math::mix64`), TPDF = their
   difference (same variance, 1/6 LSB²); on the 64th zero it clears `err`, resets `n = 0`
   and caps `zeros` at 64; and a **fresh ditherer starts in that state** (zeros = 64), so
   rest ≡ fresh exactly (`Ditherer::at_rest()`, `PartialEq`) and silence at the start of a
   render stays silent. Settle points then exist for Terminus/Culmen with dither on. Tests:
   rest = fresh after sound + 64 zeros for all three shapings, and from rest the output is
   bit-identical to a fresh unit's; mean/variance/lag-1/cross-channel statistics. A
   `DSP_VERSION` 3 bump (no current golden dithers: `v5.cantus` and `v14.cantus` name no
   limiter or maximizer, and the Pulsus I project is built in code without one).
2. **The `f32` `powf` fast path** above (a `DSP_VERSION` bump), then re-measure the kernels
   on a quiet machine.
3. **Graph's 7 sites** (`docs/round21/requests.md`).
4. **`_set_FMA3_enable(0)`** in `cantus-rt` (rt's interim, R9 (2)) can go once this merges:
   no rendered number reaches the CRT's transcendentals any more.
5. **CI across machine classes** (R9 (4)): run `cargo test -p cantus-dsp --lib math::` and the
   null tests on an aarch64 runner and a second x86 class; the pinned digests and hashes
   must agree. The render cache key can then drop the CPU class.

## Notes for the integrator

- Merge last, as planned: `af74c7b` re-blesses `GOLDEN_V5`, `GOLDEN_V1_KIT`,
  `GOLDEN_V14_KIT`, `GOLDEN_V1_FROM_BAR_2`. If another branch changes bits first, re-run
  with `CANTUS_GOLDEN_DUMP=<dir>` before and after and re-bless once.
- Conflicts to expect are one-liners: `render.rs` (the normalise gain → `math::powf`), the
  `use cantus_dsp::math;` lines, and `cantus-engine/tests/*` where at-rest added tests (any
  new std call there only warns).
- New dependency: `libm =0.2.16` (already in `Cargo.lock` through other crates; no
  download). `cantus-core` now depends on `cantus-dsp`.
- `[profile.dev.package.libm] opt-level = 3`, like `cantus-dsp`'s (bits are identical at any
  opt level — no fast-math, no FMA contraction).

## Appendix — kernel A/B (ns/frame, the lower median of two alternated runs each; noisy, see above)

| Kernel | std (`654a67a`) | libm (`af74c7b`) | change |
|---|---:|---:|---:|
| fx.aequalis.default | 0.8 | 0.8 | +0.0 % |
| fx.aequalis.heavy | 115.6 | 67.2 | -41.9 % |
| fx.comprimo.default | 214.1 | 175.0 | -18.2 % |
| fx.comprimo.heavy | 632.0 | 571.9 | -9.5 % |
| fx.terminus.default | 909.8 | 743.0 | -18.3 % |
| fx.terminus.heavy | 1003.5 | 630.5 | -37.2 % |
| fx.spatium.default | 1146.5 | 1135.2 | -1.0 % |
| fx.spatium.heavy | 1193.4 | 1137.5 | -4.7 % |
| fx.multiplex.default | 5.5 | 5.5 | +0.0 % |
| fx.multiplex.heavy | 823.8 | 1066.8 | +29.5 % |
| fx.sibilus.default | 320.7 | 374.6 | +16.8 % |
| fx.sibilus.heavy | 358.6 | 750.0 | +109.2 % |
| fx.porta.default | 52.3 | 73.4 | +40.3 % |
| fx.porta.heavy | 67.2 | 164.8 | +145.3 % |
| fx.fervor.default | 14.8 | 14.8 | +0.0 % |
| fx.fervor.heavy | 603.5 | 738.7 | +22.4 % |
| fx.latitudo.default | 50.0 | 64.8 | +29.7 % |
| fx.latitudo.heavy | 50.0 | 39.8 | -20.3 % |
| fx.culmen.default | 1055.1 | 1354.3 | +28.4 % |
| fx.culmen.heavy | 1053.5 | 1453.1 | +37.9 % |
| fx.delay.default | 25.0 | 40.6 | +62.5 % |
| fx.delay.heavy | 30.5 | 40.6 | +33.3 % |
| fx.reverb.default | 64.1 | 82.0 | +28.0 % |
| fx.reverb.heavy | 61.7 | 60.9 | -1.3 % |
| inst.firmus.v1 | 161.7 | 122.7 | -24.2 % |
| inst.firmus.v8 | 985.9 | 498.4 | -49.4 % |
| inst.firmus.v32 | 2027.0 | 937.9 | -53.7 % |
| inst.pulsus.v1 | 1299.6 | 1459.8 | +12.3 % |
| inst.pulsus.v8 | 1299.2 | 1640.6 | +26.3 % |
| inst.pulsus.v32 | 5124.2 | 4573.0 | -10.8 % |
| inst.unda.v1 | 179.7 | 147.7 | -17.8 % |
| inst.unda.v8 | 1065.6 | 1329.7 | +24.8 % |
| inst.unda.v32 | 2779.3 | 1583.6 | -43.0 % |
| inst.calor.v1 | 428.9 | 528.9 | +23.3 % |
| inst.calor.v8 | 3666.0 | 3671.9 | +0.2 % |
| inst.calor.v32 | 3933.2 | 3610.9 | -8.2 % |
| inst.modulus.v1 | 108.6 | 63.3 | -41.7 % |
| inst.modulus.v8 | 290.6 | 430.9 | +48.3 % |
| inst.modulus.v32 | 979.7 | 834.8 | -14.8 % |
| inst.exemplar.v1 | 250.8 | 189.8 | -24.3 % |
| inst.exemplar.v8 | 3432.4 | 2574.2 | -25.0 % |
| inst.exemplar.v32 | 22916.4 | 15316.4 | -33.2 % |
| inst.gravis.v1 | 232.0 | 136.7 | -41.1 % |
| inst.gravis.v8 | 240.6 | 162.5 | -32.5 % |
| inst.gravis.v32 | 229.7 | 150.0 | -34.7 % |
| inst.lamina.v1 | 214.1 | 150.8 | -29.6 % |
| inst.lamina.v8 | 700.0 | 525.8 | -24.9 % |
| inst.lamina.v32 | 1940.2 | 1858.2 | -4.2 % |
| inst.organum.v1 | 831.2 | 747.3 | -10.1 % |
| inst.organum.v8 | 841.8 | 775.4 | -7.9 % |
| inst.organum.v32 | 943.4 | 903.9 | -4.2 % |
