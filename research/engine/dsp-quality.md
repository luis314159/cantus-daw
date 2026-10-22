# DSP quality and efficiency where the engine owns it

*Round 20, engine-research, 2026-10-06. For ADR 0031's review. Sources are marked
**[fetched]** (the page was read) or **[excerpt]** (seen only as a search-result excerpt; the
Zscaler proxy blocked or truncated some). Claims without a mark are engineering judgement
and say so. Cantus is GPL (ADR 0025 §10, ADR 0001), which matters for licences below.*

## 0. Where Cantus already stands (read in the code, 2026-10-06)

The DSP layer is not the weak point; it is closer to the state of the art than the engine
around it. What the code does today:

| Area | Cantus today | Verdict against the literature |
|---|---|---|
| File SRC on load | `cantus-engine/src/resample.rs`: exact rational positions, Kaiser-windowed sinc, β = 10 (~100 dB), 32 zero crossings, cutoff at 95 % of the narrower Nyquist, phases normalised to unity DC, kernels in `f64` | Good, but below export-grade converters (SoX VHQ, r8brain: 150–218 dB stop band). Fine for 24-bit material. §1 |
| Sampler varispeed | `cantus-dsp/src/exemplar_resample.rs`: octave mip-maps + polyphase windowed sinc at 2× + half-band decimator, after de Soras, *The Quest for the Perfect Resampler* | State of the art for real-time varispeed (it is soxr's lineage too) |
| True peak | `terminus_os.rs`: 4× Kaiser polyphase, phase 0 a pure delay, **parabolic refinement between oversampled points** | Better than BS.1770 Annex 2's plain 4× meter, which under-reads by up to 0.688 dB near fs/2 (§8) |
| Loudness | `terminus_loudness.rs`: BS.1770-4/R128 on 10 ms columns, histogram gating in 0.01 LU bins with exact energy sums, no allocation | Finer than libebur128's 0.1 dB histogram; matches the efficient method. Update the reference to BS.1770-**5** (2023) |
| Dither | `terminus.rs` `Ditherer`: TPDF + error-feedback shaping, **sequential xorshift** seeded per channel; silence after 64 zeros | Quality is right; the *state* is a problem for ADR 0031 (§7 below) |
| Oversampling | `fervor_os.rs`: 2/4/8× half-band cascades, linear-phase FIR **or** polyphase IIR all-pass; `comprimo_oversample.rs`: FIR 2/4× | State of the art in structure (de Soras HIIR style); ADAA would let the default drop a factor (§3) |
| Smoothing | `smoother.rs`: one-pole, coefficient `(-4.6/n).exp()` | Fine as a shape; the `exp` is the platform's (§9) |
| Transcendentals | ~450 calls of std `sin/exp/powf/tanh` in `cantus-dsp/src` | **Cross-machine determinism hazard** on Windows (§9) |

## 1. Sample-rate conversion

**Methods.** J. O. Smith's bandlimited interpolation — a windowed-sinc table with
interpolation between table entries, quality set by length, table oversampling and
coefficient precision — is the ancestor of nearly all of them (J. O. Smith III, *Digital
Audio Resampling Home Page*, CCRMA, rev. 2020,
https://ccrma.stanford.edu/~jos/resample/ **[fetched]**). libsamplerate (E. de Castro Lopo,
BSD-2, v0.2.2, https://github.com/libsndfile/libsamplerate **[fetched]**) implements it with
a variable ratio. libsoxr (R. Sykes) combines it with FFT oversampling, inspired by
de Soras; presets VHQ…QQ (https://sources.debian.org/src/libsoxr/0.1.3-4/README/
**[excerpt]**). r8brain-free-src (A. Vaneev, MIT, https://github.com/avaneev/r8brain-free-src
**[fetched]**) oversamples 2× then interpolates a bank of fractional-delay sinc filters;
transition band 0.5–45 %, stop band 49–218 dB, minimum-phase option. Infinite Wave's
comparisons (https://src.infinitewave.ca/ **[fetched]**, 400+ converters, sweeps at −0.1
and −60 dBFS) put SoX VHQ, r8brain and iZotope RX "steep" at the top.

**Rust.** `rubato` 5.0.1 (MIT/Apache, https://docs.rs/rubato/latest/rubato/ **[fetched]**):
`Async` (sinc or polynomial), `Fft` (fixed ratio, fastest), `Slip`; real-time safe after
construction with `process_into_buffer`. Two caveats for Cantus: its sinc path **dispatches
AVX/SSE3/Neon at run time** (different summation orders across CPUs), and a ratio ramp is
spread over one chunk, so output depends on chunk length (it would break block-size
independence unless fed a fixed internal quantum aligned to absolute sample index).

**For Cantus.** (a) Real-time clip varispeed and warping playback: Exemplar's design
(mip-maps + polyphase sinc at 2× + half-band) is already the right one; reuse it for audio
clips when warping lands, with the phase as a 64-bit fixed-point function of the absolute
timeline sample, never accumulated per block. (b) Import/export: add an **export-grade
fixed-ratio path** (≥ 150 dB, linear phase, transition just above 20 kHz), used when a file
is imported at another rate and for exports at another rate than the session's; keep the
current 100 dB kernel as the fast path. Validate against Infinite Wave's test signals.

## 2. Time-stretch and pitch-shift (for ADR 0025's "later: warping")

**Literature.** Phase locking: J. Laroche, M. Dolson, "Improved phase vocoder time-scale
modification of audio", *IEEE Trans. Speech and Audio Processing* 7(3), 1999
(https://www.ee.columbia.edu/~dpwe/papers/LaroD99-pvoc.pdf **[excerpt]**). Phase-gradient
heap integration (PGHI): Z. Průša, N. Holighaus, "Phase vocoder done right", EUSIPCO 2017,
arXiv:2202.07382 (https://arxiv.org/abs/2202.07382 **[fetched]**) — no peak picking, no
transient detection, artefact-free at large factors; and Z. Průša, P. Balazs, P. L.
Søndergaard, "A noniterative method for reconstruction of phase from STFT magnitude",
*IEEE/ACM TASLP* 25(5), 2017 (arXiv:1609.00291 **[excerpt]**). Survey: J. Driedger, M.
Müller, "A review of time-scale modification of music signals", *Applied Sciences* 6(2):57,
2016 (https://www.mdpi.com/2076-3417/6/2/57 **[excerpt]**). Transients: J. Driedger, M.
Müller, S. Ewert, "Improving time-scale modification of music signals using
harmonic-percussive separation", *IEEE SPL* 21(1), 2014 — harmonic part through a long-frame
phase vocoder, percussive part through short-frame OLA
(https://www.audiolabs-erlangen.de/resources/MIR/2014-SPL-HPTSM/ **[excerpt]**).

**Libraries.** Rubber Band (Breakfast Quay; GPL-2.0-or-later or commercial; R3 "finer"
engine since v3.0, better on mixes, vocals and bass at much higher CPU; "RT-safe in
real-time mode … no allocation, locking or blocking after initialisation";
https://breakfastquay.com/rubberband/ **[fetched]**). zplane élastique: proprietary, licensed
by Live, Cubase, FL Studio (https://licensing.zplane.de/technology **[fetched]**).
Signalsmith Stretch (G. Luff, **MIT**, from his ADC22 talk "Four ways to write a
pitch-shifter"; https://github.com/Signalsmith-Audio/signalsmith-stretch **[fetched]**).
Bungee (Parabola Research, MPL-2.0; https://github.com/bungee-audio-stretch/bungee
**[fetched]**).

**For Cantus.** (Superseded by ADR 0061 §1: since ADR 0047 the engine crates are MPL-2.0, so GPL code such as Rubber Band can only be *heard* as a reference; Cantus has its own stretcher.) Rubber Band R3 is the quality reference to listen to and
Signalsmith Stretch (MIT) as the light, embeddable default. The engine-level rule matters
more than the library: **warped clips are rendered into the render cache (ADR 0031 §4) and
played as audio**; the real-time stretcher runs only for what is being edited (preview),
exactly like a node that is "live". Stretch parameters change only at hop boundaries
derived from absolute timeline position, so a warped render is block-size independent and
cacheable. Long term, a Rust PGHI phase vocoder with a harmonic/percussive split is the
open state of the art.

## 3. Oversampling and antialiasing

**ADAA.** J. Parker, V. Zavalishin, E. Le Bivic, "Reducing the aliasing of nonlinear
waveshaping using continuous-time convolution", DAFx-16
(https://dafx.de/paper-archive/details/vem_XXF5qBbfiWOH2RVVAA **[excerpt]**); S. Bilbao,
F. Esqueda, J. Parker, V. Välimäki, "Antiderivative antialiasing for memoryless
nonlinearities", *IEEE SPL* 24(7), 2017 **[excerpt]**; M. Holters, "Antiderivative
antialiasing for stateful systems", DAFx-19, best paper
(https://www.hsu-hh.de/ant/wp-content/uploads/sites/699/2020/10/DAFx2019_paper_4.pdf
**[excerpt]**); D. Albertini, A. Bernardini, A. Sarti, "Antiderivative antialiasing in
nonlinear wave digital filters", DAFx-20 **[excerpt]**; V. Zheleznov, S. Bilbao,
"Interpolation filters for ADAA", DAFx-24 **[fetched, title only]**. Practical notes:
J. Chowdhury, *ADAA notebook* (https://ccrma.stanford.edu/~jatin/Notebooks/adaa.html
**[fetched]**): first order leaves noticeable aliasing, second order much less; the
divided difference is ill-conditioned when consecutive inputs are close (fallback with a
fixed tolerance); ADAA adds half a sample of delay; **first-order ADAA with 2× oversampling
beats either alone**.

**Oscillators.** V. Välimäki, A. Huovilainen, "Antialiasing oscillators in subtractive
synthesis", *IEEE Signal Processing Magazine* 24(2), 2007 **[excerpt]**; E. Brandt, "Hard
sync without aliasing", ICMC 2001 (minBLEP) **[excerpt]**; V. Välimäki, J. Pekonen, J. Nam,
"Perceptually informed synthesis of bandlimited classical waveforms using integrated
polynomial interpolation", *JASA* 131(1), 2012
(https://mac.kaist.ac.kr/pubs/ValimakiPeknenNam-jasa2012.pdf **[excerpt]**).

**For Cantus (efficiency, ADR 0031 §5.3).** The ADR expects the oversampling half-bands of
Comprimo and Fervor at 4× to top the bench's hot-spot list and plans SIMD for them. The
literature offers a cheaper first move: **ADAA in Fervor's shapers (and any clipper),
which lets the default drop from 4× to 2×** for equal or lower aliasing, halving the
oversampled work and the latency before a single SIMD lane is written. It changes the
sound, so it is a reviewed `DSP_VERSION` bump with new goldens, offered as the new default
with the old modes kept. The ADAA fallback branch is data-dependent but deterministic.

## 4. Zero-delay-feedback filters and modulation

V. Zavalishin, *The Art of VA Filter Design*, Native Instruments, rev. 2.1.2, 2020
(https://www.native-instruments.com/fileadmin/ni_media/downloads/pdf/VAFilterDesign_2.1.0.pdf
**[excerpt]**) — the topology-preserving transform; A. Simper (Cytomic), technical papers
*SvfLinearTrapOptimised2* and others (https://cytomic.com/technical-papers/ **[fetched]**);
A. Wishnick, "Time-varying filters for musical applications", DAFx-14 — a proof that the
trapezoidal SVF stays stable under per-sample parameter variation, unlike interpolated
direct-form biquads **[excerpt]**. For Cantus: the trapezoidal SVF as the modulated filter
everywhere; coefficients from a **deterministic** `tan` (§9).

## 5. SIMD voice processing

Surge XT's `QuadFilterChainState` packs four voices into SSE lanes with per-block
coefficient deltas (https://github.com/surge-synthesizer/surge/blob/main/src/common/dsp/QuadFilterChain.h
**[fetched]**); Vital's `poly_float` is four lanes on SSE2/Neon
(https://github.com/mtytel/vital/blob/main/src/synthesis/framework/poly_values.h
**[fetched]**). Both are evidence for ADR 0031 §5.3's structure-of-arrays voice banks.
The determinism argument of the ADR is correct and the literature states its limits
exactly: IEEE add/mul/div/sqrt are correctly rounded per lane, so vertical SIMD equals
scalar; bits move only with (1) horizontal reductions, (2) FMA vs mul+add, (3) approximate
instructions (`rcpps`, `rsqrtps` — which also differ between Intel and AMD), (4) library
math. **Amendment:** add (3) to the ADR's rules — no approximate reciprocal instructions in
any kernel — and (4), which §9 covers.

## 6. Parameter smoothing

JUCE `SmoothedValue` (linear and multiplicative ramps,
https://docs.juce.com/master/classSmoothedValue.html **[fetched]**); A. Simper, "Dynamic
smoothing" (a two-pole SVF whose cutoff follows the input's speed, Cytomic) **[excerpt]**;
Surge interpolates coefficients linearly across a block. For Cantus: smooth in the
perceptual domain (dB, log-frequency, pan angle), per sample; when coefficients are
recomputed every *N* samples for cost, *N* is a **fixed internal sub-block aligned to the
absolute sample index**, never the host buffer — which is what keeps the block-size
contracts true by construction rather than by each unit's care.

## 7. Dithering and noise shaping — and why the ADR should care

S. P. Lipshitz, R. A. Wannamaker, J. Vanderkooy, "Quantization and dither: a theoretical
survey", *JAES* 40(5), 1992 — TPDF at 2 LSB peak-to-peak makes the error's first two moments
independent of the signal
(https://hajim.rochester.edu/ece/sites/zduan/teaching/ece472/reading/Lipshitz_1992.pdf
**[fetched]**); M. Gerzon, P. Craven, "Optimal noise shaping and dither of digital signals",
AES Convention 87, 1989 (https://aes2.org/publications/elibrary-page/?id=6647
**[excerpt; 403]**); R. A. Wannamaker, "Psychoacoustically optimal noise shaping", *JAES*
40(7/8), 1992 **[excerpt]**. Cantus's `Ditherer` is TPDF with error-feedback shaping: right.

**The engine problem.** Its noise comes from a sequential xorshift whose state advances
only on non-zero samples. So a dithering Terminus or Culmen on the master is **never at
rest** in the sense of ADR 0031 §5.1 (its state is never a fresh unit's), which means:
the master chain has **no settle points** for the render cache (§4.2) or time-parallel
export (§7.3) whenever dither is on — and dither is on in every 16/24-bit export.
**Amendment:** a **counter-based** generator — the noise for channel *c* at absolute sample
*n* is `hash(seed, c, n)` (SplitMix64 or a Philox round) — makes the dither stateless: the
same noise whatever the block size, start point, thread or split, and leaves only the
shaping filter's few samples of error state, which do come to rest after 64 zeros as
today. It changes the noise sequence (a `DSP_VERSION` bump), not its statistics.

## 8. Loudness and true peak

ITU-R BS.1770 is now at **revision 5** (11/2023; https://www.itu.int/rec/R-REC-BS.1770
**[fetched]**); EBU R 128 at **v5.0** (11/2023; https://tech.ebu.ch/publications/r128
**[fetched]**), metering in EBU Tech 3341, LRA in Tech 3342. Annex 2 asks for ≥ 4×
oversampling, which under-reads a tone near fs/2 by up to 20·log10(cos(π/8)) = 0.688 dB
(arithmetic; figure from https://www.forasoft.com/learn/audio-for-video/articles-audio/true-peak-dbtp-inter-sample
**[excerpt]**). libebur128 (J. Kokemüller, MIT, https://github.com/jiixyj/libebur128
**[fetched]**) uses a 49-tap 4× polyphase FIR below 96 kHz and, in histogram mode, 1 000
bins of 0.1 dB; the Rust `ebur128` crate (S. Dröge, MIT) is a faithful port passing
Tech 3341/3342 (https://docs.rs/ebur128/latest/ebur128/ **[fetched]**). Cantus's meter
already exceeds both (0.01 LU bins with exact sums; parabolic peak refinement). Only the
references need updating; the conformance tests should add BS.1770-5's cases.

## 9. Floating-point determinism across machines (the ADR's risk 6, made concrete)

- **Rust's float semantics** (RFC 3514, https://rust-lang.github.io/rfcs/3514-float-semantics.html
  **[fetched]**): IEEE round-to-nearest, no FTZ, **no contraction of `a*b+c` into FMA** —
  so ADR 0031 §5.3's v2 = v3 claim holds for Cantus's own arithmetic. But std's `sin`,
  `exp`, `powf`, `tan` have *unspecified precision* that "varies by platform, Rust version,
  and can even differ within the same execution" (https://doc.rust-lang.org/std/primitive.f64.html
  **[fetched]**); they call the platform's libm.
- **Microsoft's UCRT** chooses at start-up between FMA3 and non-FMA3 implementations of the
  transcendental functions: "Slight differences in the result of computations may be
  observable … between computers that do or don't support FMA3"
  (`_get_FMA3_enable`/`_set_FMA3_enable`,
  https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/get-fma3-enable-set-fma3-enable
  **[fetched]**). So the **same Cantus binary** renders different bits on a pre-Haswell CPU
  than on a modern one — and the ADR's v2-vs-v3 CI job cannot see it, because both builds
  run on the same CI machine and call the same UCRT path.
- **Changing MXCSR is undefined behaviour in Rust's model** unless restored inside the same
  asm block; `_mm_setcsr` is deprecated since Rust 1.75 for that reason (RFC 3514;
  https://doc.rust-lang.org/core/arch/x86/fn._mm_setcsr.html **[excerpt]**). LLVM may
  constant-fold or reorder floating-point code assuming the default environment.

**Amendments.** (1) All DSP transcendentals go through one deterministic module: the
pinned pure-Rust `libm` crate (MIT, https://docs.rs/libm/latest/libm/ **[fetched]**) or
Cantus's own polynomials, with std's methods banned in `cantus-dsp`/`cantus-engine` by
clippy's `disallowed_methods`. Coefficient code is not hot, so the cost is small; per-sample
`tanh`/`exp` in shapers already want polynomials. This is one `DSP_VERSION` bump and makes
"same snapshot ⇒ same bits" true **across machines**, which the render cache (shared
across projects, possibly synced) and published goldens need. Until then, call
`_set_FMA3_enable(0)` at start-up on Windows x64 (FFI in `cantus-rt`) so every Windows
machine takes one path. (2) The FTZ/DAZ guard of §5.2 stays (hosted plug-ins expect it,
and the bench measured the speed), but the ADR should say what it rests on: Cantus's DSP
never produces a subnormal that matters (it flushes in software), so FTZ changes no
sample *and* the compiler's default-environment assumption is never contradicted in a way
that changes output; the null corpus with the guard on and off is the test. (3) Add a CI
render on a second machine class (or under an emulator flag that disables FMA3) and an
aarch64 runner, comparing null-test hashes across machines, not only across builds.
