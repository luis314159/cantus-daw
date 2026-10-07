# Round 22 — *math2*: a fast `f32` `powf`, dither that rests (R10), and Aequalis's residual

Branch `r22/math2`, base c8bc9f2. ADR 0031 R9 (follow-up), R10, R14's spirit (cheaper DSP
before faster DSP); `docs/round21/math.md` "Left" 1–2; `docs/round21/at-rest.md` finding 3.
**One** change of bits: `DSP_VERSION` 3.

## What was done

1. **`math::powf` in `f32` is our own** (`crates/cantus-dsp/src/math_powf.rs`), for finite
   `x > 0` and finite `y`: `x^y = 2^(y · log2 x)` with both halves in `f64`, each a table
   step and a short polynomial in the shape of ARM optimized-routines' `powf`, carried to
   more terms because the target is correct rounding rather than 0.82 ulp:
   - `log2`: `x = 2^k · z`, `z` in `[0.699, 1.398)` (ARM's offset, which puts 1 in the
     middle of an interval); 64 intervals by the top mantissa bits; per interval `1/c` cut
     to 29 significant bits (so `z · (1/c)` is exact) and `−log2` of it; then
     `log2(1 + r)` with `|r| < 2⁻⁷` by a degree-6 polynomial — no division. Measured
     relative error 2⁻⁴⁴·⁸ over every binade and bit by bit around 1, where the interval's
     entry is exactly `(1, 0)` so the error stays relative.
   - `exp2`: `t = n/32 + f`, `|f| ≤ 1/64`, `2^(j/32)` from a 32-entry table with `n>>5`
     added to its exponent, `2^f` by a degree-5 polynomial. Measured 2⁻⁴⁸·³.
   - `y · log2 x ≥ 128` → `+∞`, `≤ −150` → `0` (ties-to-even at 2⁻¹⁵⁰), otherwise one
     rounding of the `f64` result to `f32`.
   - Every other input (zeros, negative bases, infinities, NaNs) goes to `libm::powf`
     unchanged, so IEEE 754's special cases are round 21's to the bit.
   - Tables and coefficients are **computed by `const` evaluation** (the Taylor series'
     exact rationals; 30-term series for the table entries) — IEEE arithmetic in the
     compiler, the same bits whatever machine builds them; no hand-copied hex. At run time
     plain `f64` adds and multiplies in a fixed order (Rust never contracts into FMA):
     deterministic on every machine, like the rest of `cantus_dsp::math`. The pin test
     (`the_bits_are_the_pinned_ones`) gains `powxy32`, `powwide32`, `db_gain32`; debug and
     release give the same digests.
2. **`math::db_to_gain(db)`** = `10^(db/20)` in `f32` with only the `exp2` half (the
   logarithm is the constant `log2(10)/20`, and the exponent is never rounded to `f32` on
   the way). Used at the per-sample gains: Porta's smoothed gain (`porta.rs`), Sibilus's
   reduction (`sibilus.rs`), Terminus's two gain stages (`terminus.rs`, and so Culmen's
   brickwall) and Spatium's ducker (`spatium.rs`). Every other `powf(10, …)` is coefficient
   code and keeps `powf` (now the fast one too).
3. **R10 — counter-based TPDF dither** (`terminus.rs` `Ditherer`, used by Culmen): the
   design `docs/round21/math.md` left ready. `Ditherer { seed, n, err, zeros }`; the noise
   for the `n`-th sample since the ditherer last rested is `mix64(seed + n·γ)` (SplitMix64's
   output, `va::math::mix64`), cut into two 24-bit uniforms whose difference is the TPDF
   noise (variance 1/6 LSB², as before). On the 64th zero in a row it clears the shaping
   error and sets `n = 0`; a **new ditherer starts in that state**, so rest ≡ new
   (`Ditherer::at_rest`, `PartialEq`). Seeds pass through `mix64`; Culmen's right channel
   no longer shares Terminus's left seed. Terminus's and Culmen's `rest_blocker` now say
   `"dither"` only while a ditherer is not at rest. A side effect, on purpose: digital
   silence at the start of a render stays silent (the sequential generator dithered the
   first 63 zeros).
4. **Aequalis's dynamic residual** (at-rest finding 3), which turned out to be three
   things, each found by the twin test once the one before it was fixed:
   - a dynamic band's update skips moves under 0.0001 dB, so its release ended a hair from
     0 and stayed. Now a delta of exactly 0 always lands (as `+0.0`) and redesigns the band
     once;
   - in **linear phase** a dynamic band runs its movement as biquad sections only while it
     moves; when the delta returned to 0 the section count dropped to 0 and the sections'
     state stayed, stale, never ticked and never cleared — to be resumed at the band's next
     movement. A new unit's is zero, so the unit was never "as good as new". `redesign` now
     clears every section no longer in use;
   - the band's **detector** (a band-pass biquad) flushed its two state variables one at a
     time below 1e-20, and the pair hovered at the threshold for ever — exactly round 21's
     finding 2 in the meters and Multiplex's detectors. `State::tick` now flushes both at
     once. (This is Aequalis's audio-path biquad too; it moves only the last 1e-20 of a
     tail, and nothing in the goldens.)
   With the three, the heavy EQ (eight bands, two dynamic, linear phase at its finest) comes
   to rest in 10.6–10.7 s of silence with FTZ, and the contract test now requires it.
5. **`DSP_VERSION` 3**, one commit re-blessing every golden (below).

## The one change of bits: `DSP_VERSION` 3

Size of the change, `|new − old|` over each golden's `f32` render (`CANTUS_GOLDEN_DUMP` at
c8bc9f2 and at this branch, compared sample by sample):

| Golden | Samples | Differing | Max abs diff | dBFS | re the golden's peak |
|---|---:|---:|---:|---:|---:|
| `v5.cantus` (`null_tests` `GOLDEN_V5`) | 1 968 000 | 1.2 % | 3.0·10⁻⁸ | −150.5 | −128.9 dB |
| Pulsus I kit (`GOLDEN_V1_KIT`) | 794 182 | 34.7 % | 6.0·10⁻⁸ | −144.5 | −135.6 dB |
| Pulsus I kit from bar 2 (`GOLDEN_V1_FROM_BAR_2`) | 571 638 | 34.9 % | 6.0·10⁻⁸ | −144.5 | −136.3 dB |
| v14 fixture kit (`GOLDEN_V14_KIT`) | 288 000 | 0 | 0 | — | unchanged |

At most one `f32` ulp of a sample near ±0.5 (2⁻²⁴): coefficient powers that moved by an
ulp (round 21's libm `powf` was ≤ 0.8 ulp, ours is correctly rounded), carried into the
render. Nothing else in the four goldens dithers or uses a dynamic EQ band (as round 21
noted, none names a limiter or a maximizer), so the dither and Aequalis changes do not
show in them; they are proven by their own tests. The math pin test's `pow10_32` digest
moved and three digests were added; every other function's digest is unchanged.

## Accuracy and speed of the `f32` `powf`, measured

`math_report` (release, pinned to logical CPU 4, a P-core of the i7-12700H, UCRT with
FMA3, the machine quiet at the time), 4000 points per case against mpmath at 200 bits.
Error in `f32` ulps; ns per call with an indirect call's cost subtracted.

| Case | UCRT max | libm (r21) max | **ours max** | ours = UCRT bits | UCRT ns | libm ns | **ours ns** |
|---|---:|---:|---:|---:|---:|---:|---:|
| powf 10^[−7.5, 2] | 0.500 | 0.789 | **0.500** | 100.0 % | 7.0 | 36.8 | **10.5** |
| powf 2^[−10, 10] | 0.500 | 0.742 | **0.500** | 100.0 % | 6.4 | 37.1 | **10.5** |
| powf (0, 1]^[0.1, 10] | 0.500 | 0.803 | **0.500** | 100.0 % | 7.7 | 42.2 | **10.8** |
| powf [1e−30, 1e30]^[−4, 4] | 0.857 | 0.780 | **0.499** | 99.9 % | 25.7 | 47.5 | **14.5** |
| powf [0.999, 1.001]^[−1e4, 1e4] | 0.500 | 0.765 | **0.500** | 100.0 % | 8.9 | 41.2 | **10.8** |
| `db_to_gain` [−150, 24] dB (vs `powf(10, dB/20)`) | 7.73 | 7.73 | **0.500** | 35.3 % | 8.5 | 36.4 | **3.0** |
| candidate: libm's `exp2(y·log2 x)` in f64, 10^[−7.5, 2] | | | 0.500 | 100.0 % | | | 16.0 |

- **Accuracy**: correctly rounded wherever it was measured — the 20 000 report points and
  1.4 M test points (`powf_is_within_half_an_ulp`: dB levels, octaves, curve exponents,
  the whole `f32` range, bases near 1 with huge exponents, subnormal bases, and results at
  the overflow and underflow edges) never once gave another `f32` than `pow` in `f64`
  rounded. The kernels: `log2` 2⁻⁴⁴·⁸ relative, `exp2` 2⁻⁴⁸·³. The UCRT's `powf` is not
  better (0.857 ulp over the wide case); round 21's libm was 0.74–0.80 ulp.
- **Speed**: 3.5–4× faster than round 21's libm `powf`, 1.3–1.6× the UCRT's FMA3 path.
  `db_to_gain` (the `exp2` half alone) is 3 ns, faster than the UCRT's `powf(10, x)`. The
  `7.73 ulp` of the other two columns is not their `powf`'s fault: `powf(10, dB/20)`
  rounds `dB/20` to `f32` first, which `db_to_gain` never does.
- Round 21's recommended candidate (libm's own `f64` `exp2`/`log2`) is as accurate and
  ~16 ns; the table-free series kernels I tried first (one division, 8 + 11 terms) were
  26–34 ns under load. Both were measured and dropped for the table version.
- Earlier runs under load (other builders compiling) gave UCRT 13–20, libm 69–95, ours
  17–31 ns: the ratios held.

## On the kernels (paired A/B, pinned)

`cantus-bench --quick --only fx,inst` of the base (c8bc9f2) and of this branch, built from
this tree, run alternately on logical CPU 4 (`start /affinity 10`, high priority): six fx+inst
pairs (three base-first, three branch-first) and four more ABBA fx pairs late at night.
**The machine is shared with the other builders**: untouched kernels (Delay, Reverb,
Latitudo, Fervor) moved ±20 % from pair to pair, the instruments up to ±60 %. So each
pair is normalised by the median ratio of those untouched controls; ns/frame medians:

| Kernel | Median pair ratio | Normalised by the controls | Range over 10 pairs (normalised) |
|---|---:|---:|---|
| fx.porta.heavy | −37.7 % | **−36.9 %** | 0.52–0.74 |
| fx.sibilus.default | −29.5 % | **−28.9 %** | 0.59–0.86 |
| fx.sibilus.heavy | −26.4 % | **−25.7 %** | 0.63–0.89 |
| fx.terminus.default | −4.6 % | −7.1 % | 0.73–1.25 |
| fx.terminus.heavy | −1.6 % | −4.7 % | 0.78–1.35 |
| fx.multiplex.heavy | −5.9 % | −6.8 % | 0.80–1.23 |
| fx.comprimo.heavy | −7.5 % | −6.5 % | 0.71–1.30 |
| fx.porta.default | −6.3 % | −5.0 % | 0.81–1.13 |
| fx.aequalis.heavy | −5.3 % | −4.9 % | 0.68–1.32 |
| fx.spatium.default / heavy | −8.0 / −6.7 % | −4.9 / −3.4 % | 0.76–1.23 |
| fx.culmen.default / heavy | −0.4 / +1.2 % | −1.4 / +1.5 % | 0.93–1.10 |

Porta's and Sibilus's per-sample gain was most of their cost and is now a third to a
quarter cheaper. Terminus pays the gain stage's `10^(−gr/20)` only while it reduces, and
Culmen's cost is its band stage, so they move within the noise. No instrument calls the
changed paths per sample (their `powf` calls are in settings and note-on code, now
cheaper); their pair ratios scattered from 0.16 to 6.1 and are noise. Raw tables:
`target/ab/` on the builder's machine; reproducible with the commands above.

`cargo run -p xtask -- perf --only fx,inst --pin 4` on the branch (quiet machine) against
the committed round-20 baseline: "no regression beyond 25 %", every metric faster — which
measures that night's quiet machine more than the branch; the A/B above is the
measurement.

## Rest, measured (the twin tests)

`fx_contract`'s `at_rest::*` (feed, silence until `at_rest()`, then four twins — continuing,
skipped, reset, new — must render the same bits; FTZ trials marked). New this round: the
limiter and the maximizer are checked at three more settings, dithering to 16 bits at each
noise shaping, and must rest there; the EQ must rest at its defaults and at the heavy
settings (`must_rest`: Limiter and Eq now `(true, true)`).

| Kind | Setting | Silence before rest (three trials) |
|---|---|---|
| Limiter | heavy (TP, 5 ms look-ahead, dither 20 bits) | 9.57 / 8.07 (FTZ) / 9.57 (FTZ) s — was "no rest" |
| Limiter | dither 16 bits flat / basic / optimized | 8.0–8.3 / 7.8–9.6 / 7.2–8.4 s |
| Maximizer | dither 16 bits flat / basic / optimized | 7.0–7.8 / 7.5–8.5 / 7.6–11.4 s |
| Eq | heavy (two dynamic bands, linear phase) | no rest without FTZ (the detector's envelope has no flush, as before) / 10.72 / 10.63 s with FTZ — was "no rest" |

Every twin agreed in every trial that rested, for every kind; the reset and new twins of a
dithering limiter render the same noise as the one that rested, which is R10's point.

## Tests

- New in `cantus-dsp`: `math::fast::tests::{powf_is_within_half_an_ulp,
  the_kernels_hold_their_bounds, exact_cases_and_special_values,
  db_to_gain_is_within_half_an_ulp}`; `terminus::tests_dither::{a_ditherer_at_rest_is_a_new_one
  (all three shapings × 16/20/24 bits × four seeds), silence_first_is_silent_and_short_gaps_keep_the_stream,
  the_noise_is_white_triangular_and_independent (2²⁰ samples per seed: mean, variance 1/6,
  ¾ within ±½ LSB, lag-1 and cross-seed correlation), a_dithering_limiter_comes_to_rest_as_a_new_one}`;
  `aequalis::tests::a_dynamic_band_comes_home_exactly` (every mode). The pin test gains
  three digests and re-pins `pow10_32`; debug and release agree.
- `cargo test -p cantus-dsp`: 434 passed, 0 failed, 6 ignored. The existing
  `dither_quantises_with_the_noise_it_promises` passes unchanged (flat RMS 0.5 LSB, the
  shaped curves' bass reduction).
- `cargo test -p cantus-core -p cantus-engine -p cantus-plugins`: 510 passed, and the
  three hash goldens failed — the only failures — until the re-bless, after which they
  pass (re-run); v14's hash did not move. **`hosted_in_cantus`** — every exported CLAP/VST3 plug-in hosted by
  Cantus sounds as the native insert, to the bit — passes on `DSP_VERSION` 3.
- `cargo test -p cantus-ui`: 602 passed, 0 failed.
- `cargo clippy -p cantus-dsp -p cantus-core -p cantus-engine -p cantus-plugins --tests
  --examples`: clean.
- One unrelated flake seen once: `sched::the_serial_fallback_has_hysteresis_chase_lev`
  ("then serial") failed in one full run under heavy load and passed in the next two; it is
  the pool builder's timing test, untouched here.

## Files outside `cantus-dsp`

`crates/cantus-engine/tests/fx_contract.rs` (dithering specs, `must_rest`),
`crates/cantus-engine/tests/null_tests.rs` and `inst_pulsus.rs` (the re-blessed hashes and
their notes). No `processor*.rs`, `graph/` or `sched/` file was touched.

## Left

1. **Per-sample `20·log10(x)` in the meters** (Terminus's and Culmen's `db`, Porta's level,
   Sibilus's): libm's `log10f` is 10–15 ns; the same table `log2` kernel times
   `log10(2)` would be ~5 ns and correctly rounded. A `DSP_VERSION` bump of its own — it
   moves meter bits only where a meter feeds the audio (Porta's and Terminus's detectors
   do).
2. The `f64` `powf` is still libm's `pow` (36–48 ns): the same construction would need a
   double-double `log2` to be correctly rounded in `f64`; no per-sample caller needs it.
3. The UCRT is still 1.3–1.6× faster at `powf` (FMA3, and its kernels are tuned for the
   machine); ours could use `mul_add` (correctly rounded everywhere, so still
   deterministic) once the build builder's x86-64-v3 build makes FMA free — on v2 it would
   be a software FMA. Measure before.
4. Round 21's `_set_FMA3_enable(0)` interim in `cantus-rt` can go (math.md "Left" 4): no
   rendered number reaches the CRT's transcendentals.
5. CI across machine classes (R9 (4)) for the new pins: `cargo test -p cantus-dsp --lib
   math::` on aarch64 and a second x86 class.

## Notes for the integrator

- **Merge after pool and export if they change bits**, or re-bless once more: run the golden
  tests with `CANTUS_GOLDEN_DUMP=<absolute folder>` before and after (a relative folder is
  resolved from each crate's directory) and compare.
- `Ditherer::new` now takes a `u64` (call sites pass the same literals); Culmen's right
  channel seed changed (`0xc2b2_ae35`) so that it no longer shares Terminus's left stream.
- `math::db_to_gain` is new public API; `powf`'s signature is unchanged.
- The render cache may now key on `DSP_VERSION` 3, and settle points exist for dithering
  masters: a Terminus or Culmen that is `at_rest()` dithers the next sound exactly as a new
  one would.
