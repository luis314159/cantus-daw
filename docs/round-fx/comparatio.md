# The effects side round, wave C — `comparatio`: Comparatio, the reference comparator (DSP phase)

This is branch `fx/comparatio`, made from `fx/integration` d3b51a09. It is **DSP only**: there is
no `EffectKind`, no core parameters, no engine unit, no panel, no schema bump and no ADR number.
The name is provisional (`cantus_dsp::comparatio::NAME`). Latin *comparatio*: "a comparing, a
setting side by side".

Comparatio is a unit for the master. You load reference tracks; you switch between your mix and a
reference **at the same loudness**; you see how they differ — tonal balance, loudness and
dynamics, stereo, peaks. Its references for *function* (inspired by, never copied) are ADPTR
Metric AB, Mastering The Mix's REFERENCE and Melda's MCompare.

## Done

| Layer | Files (owned) | Shared files |
|---|---|---|
| DSP | `cantus-dsp/src/comparatio.rs`: the unit — A/B with the crossfade, the loudness match, the blind buttons, rest, `analyse_whole`. `comparatio_measure.rs`: one source's loudness, dynamics, true peak and stereo, on the audio thread. `comparatio_stereo.rs`: correlation and width, whole and in three bands. `comparatio_tonal.rs`: the long-term fractional-octave spectrum, the difference, its uncertainty and confidence. `comparatio_reader.rs`: where the reference plays (free, locked, loop) without a click. `comparatio_blind.rs`: ABX and blind preference, seeded, scored exactly. `comparatio_match.rs`: the match-EQ suggestion. `comparatio_testkit.rs`; tests `comparatio_tests.rs`, `comparatio_tests_match.rs`, `comparatio_tests_ebu.rs`, `comparatio_tests_analysis.rs`, `comparatio_tests_shared.rs`, `cantus-dsp/tests/comparatio_alloc.rs`. Example: `cantus-dsp/examples/comparatio_report.rs` (+ `comparatio_report/{cpu,write}.rs`). | `cantus-dsp/src/lib.rs`: one line in its own group, `pub mod comparatio; // fx-c:comparatio`. `comprimo_loudness.rs`: its own copy of the K-weighting replaced by Terminus's sections (marked `fx-c:comparatio`, outputs bit-identical, below). `terminus_loudness.rs`: an additive `LoudnessMeter::reset` (allocation-free; nothing else calls it). |
| Report | `C:\projects\daw-fx\compartir\comparatio\analysis.md` (written by the example, not committed) | — |
| Docs | this file | — |

### One loudness code path, and how the sharing is guarded

- **Comparatio's loudness is Terminus's** `LoudnessMeter` (`terminus::loudness`): K-weighting,
  the 10 ms columns, the 100 ms gating steps, the integrated measurement with both gates, the
  loudness range (Tech 3342), the 4× true peak. It is the code the master's meter, the export
  and Terminus's editor read. Comparatio adds nothing beside it to measure loudness: it feeds the
  meter slices that end on column boundaries, so it sees each column and each step close at its
  sample, and reads the meter there.
- **Comprimo kept its own copy** of the K-weighting (the same analog prototypes, the same
  transposed direct form II, the same flush). It is now Terminus's `KWeighting`, one per
  channel; the family has one K-weighting. Comprimo's list-based `integrated` and its 100 ms
  `Meter` stay (they are its own grid, not duplicated code).
- **Guarded:** `comparatio_tests_shared.rs` hashes Comprimo's loudness meter at 44.1, 48 and
  96 kHz (momentary, short-term, rest, each hop), its K-weighting's output, Comprimo's audio with
  the auto gain and with the loudness-matched compare (both driven by that meter) and at its
  defaults, and Terminus's meter (momentary, short-term, true peak, integrated, range). The
  hashes were taken **before** the change (commit 37d548d9) and pass after it (5cf565ab).
  Comprimo's and Terminus's own tests are green in the full suite.

## The DSP API

```rust
use cantus_dsp::comparatio::{Comparatio, ComparatioSettings, Listen, MatchMode, RefLoudness,
    analyse_whole, Heard};
use cantus_dsp::comparatio::reader::{RefAudio, Sync};
use cantus_dsp::comparatio::tonal::{Tonal, Resolution, compare};
use cantus_dsp::comparatio::match_eq::{suggest, MatchOptions};
use cantus_dsp::comparatio::blind::{Abx, Preference};

let mut c = Comparatio::new(sample_rate);       // allocates here only
c.set(&settings);                               // between blocks
c.set_transport(&transport);                    // each pass (modfx_clock::Transport)
c.set_whole_file_loudness(Some(lufs));          // from analyse_whole at load, off the audio thread
c.cue(sample);                                  // the free / loop playhead
c.process(&mut left, &mut right, Some(RefAudio { left: &rl, right: &rr, id }));
c.process_tapped(&mut l, &mut r, reference, |tap| { /* the four streams, to a worker ring */ });
c.meter();                                      // ComparatioMeter: heard, fade, gain, both Readings
c.histograms();                                 // the short-term histograms, 1 LU bins
c.restart_measurement(); c.at_rest(); c.rest_blocker(); c.skip(n); c.reset(); c.latency(); // 0

let mut t = Tonal::new(sample_rate, Resolution::Third); // the worker's, one per source
t.push(&l, &r); let curve = t.curve();
let diff = compare(&mix_curve, &ref_curve);     // mix − reference, uncertainty, confidence
let eq = suggest(&diff, &MatchOptions::default()); // (Hz, dB) points + makeup, for aequalis::fit
```

**`ComparatioSettings`** (`Default` is the null: the mix heard, untouched):

| Field | Range | Default | Meaning |
|---|---|---|---|
| `bypass` | bool | false | The input passes, to the bit; the analysers still run. |
| `listen` | `Mix`, `Reference`, `X`, `One`, `Two` | Mix | Mix and Reference are ABX's A and B; X is one of them, hidden; One and Two are the blind preference's buttons. |
| `matching` | `Integrated`, `ShortTerm`, `Manual` | Integrated | How the reference's level follows the mix's. |
| `offset_db` | −24 … +24 | 0 | Added in every mode. |
| `ref_loudness` | `Played`, `WholeFile` | Played | The integrated match reads the reference over what has played beside the mix, or over its whole file (each falls back on the other). |
| `fade_ms` | 1 … 500 | 30 | The A/B crossfade. |
| `sync` | `Free`, `Locked { offset }`, `Loop { start, end }` | Free | Where the reference plays (samples). |
| `blind_seed`, `blind_trial` | u64, u32 | 0, 0 | The blind test's order: a hash of the two. |

**`Readings`** (per source): momentary, short-term, integrated, loudness range, maximum momentary
and short-term, true peak (dBTP), sample peak, crest factor, PLR, PSR now and its mean, the
stereo bands over the whole measurement and over the last 3 s, seconds measured.

## How it works (each file's header says it in full)

- **The mix is never touched.** While the mix is heard and no fade runs, the output is the input,
  to the bit; with no reference loaded, or bypassed, always.
- **The match gain goes on the reference only**, toward the mix's loudness:
  - *Integrated*: `I_mix − I_ref`, held until both exist.
  - *Short-term*: `S_mix − S_ref` every 100 ms, held unless **both windows are whole** — every
    momentary value of the last 3 s above −70 LUFS and within 20 LU of the source's integrated
    loudness. A first version gated on the short-term value itself, and the test caught it
    raising the reference by ~16 dB as a quiet passage entered the window; the whole-window gate
    holds through the passage and its edges.
  - *Manual*: the offset alone. The gain is limited to −48 … +24 dB and glides (one-pole in dB, in
    `f64` — in `f32` a 0.5 s glide stalled short of its target, found by the rest test): 0.5 s
    integrated, 0.3 s short-term, 50 ms manual. It glides whether the reference is heard or not.
- **The switch** is an equal-power crossfade (`sin θ`, `cos θ`) counted in samples, with `θ`
  eased (`π/4·(1 − cos πx)`) so neither gain has a corner. Switching between two buttons that hide
  the same source changes nothing (no fade, no bump to give it away).
- **The reader** holds a voice — silence or a position — and whenever the position wanted is not
  where the voice would go next (a seek, a wrap, a cue, the song starting or stopping) the old
  voice continues where it would have gone and fades out while the new one fades in (5 ms,
  equal power, eased). At a loop's end the fading voice plays what the recording has after the
  region. A jump during a fade fades out the voice that was fading in from the level it had
  reached (found by the declick test: a loop's wrap 24 samples into a restart's fade-in clicked).
- **The analysers** (audio thread, per sample): Terminus's meter; the unweighted sum of squares
  and sample peak per 100 ms step (the crest factor over the steps above −70 dBFS); the true
  peak per step into a 3 s ring (PSR); a 1 LU histogram of the short-term loudness; the stereo
  bands (LR4 at 200 Hz and 4 kHz in `f64`; `ΣLL`, `ΣRR`, `ΣLR` whole and over a 3 s ring;
  correlation `ΣLR/√(ΣLL·ΣRR)`, width `1 − 2ΣLR/(ΣLL+ΣRR)`). Both sources are measured all the
  time, and their steps are aligned (one restart for both), so every pass is cut at the step's
  end and the match moves at the same sample at every block size.
- **The tonal curve** (`Tonal`, for a worker): frames under 3 Hz a bin (16 384 at 44.1/48 kHz),
  Hann, half overlapped; both channels in one complex FFT (Aequalis's table-driven FFT), taken
  apart by symmetry, their powers summed; bands of 1/3 or 1/6 octave on `1 kHz·2^(k/n)`, a bin at
  an edge counted by its overlap, scaled to the mean square in the band; frames under −70 dBFS
  not counted; each curve **relative to its own K-weighted total**, so level never shows as
  balance. Each band keeps the variance of its frame levels **relative to each frame's own
  K-weighted total** (a strum decaying is not doubt about the balance); the difference's
  uncertainty is the standard error with `n/2` frames, its confidence `1/(1 + (u/1 dB)²)`.
- **The match-EQ suggestion**: `reference − mix` per band, confidence-weighted Gaussian
  smoothing over log frequency, faded out a third of an octave beyond `lo … hi`, scaled by the
  amount and limited softly (exact up to half the limit, a tanh knee beyond, never reaching
  it), and the loudness it would add computed from the mix's K-weighted power per band and
  returned as `makeup_db`. It is a magnitude curve: Aequalis's fit (`aequalis::fit::fit`)
  turns it into editable bands in minimum or linear phase (Aequalis's choice, not ours).
- **Blind**: each trial's hidden choice is a bit of SplitMix64(seed, trial) — fair and
  independent (a balanced order would let a counting listener guess the last trials); the
  p-value is the exact binomial tail; the meter reports `Heard::Hidden` and fade 0.

## Measured

At 48 kHz unless said. `cargo test -p cantus-dsp --lib comparatio -- --nocapture`.

**Conformance (outside sources)** — the signals generated from the documents' definitions:

| Case | Read | Wanted |
|---|---|---|
| Tech 3341 #1, #2 (−23, −33 dBFS): M, S, I | −22.993, −32.993 | ±0.1 |
| Tech 3341 #3, #4, #5 (the gates, the energy mean): I | −23.014, −23.014, −22.979 | −23.0 ±0.1 |
| Tech 3341 #9 (S constant), #12 (M constant): the worst step | −22.986, −22.960 | −23.0 ±0.1 |
| Tech 3342 #1–#4: LRA | 10.00, 5.00, 20.00, 15.00 | ±1 |
| Tech 3341 #15–#19 true peak (fs/4 0°, fs/4 45°, fs/6 60°, fs/8 67.5°, +3 dBTP) | −6.000, −6.003, −6.000, −6.002, +2.997 | +0.2/−0.4 |
| … the same by BS.1770-4 Annex 2's own 48-tap interpolator | −6.197, −5.955, −6.295, −6.007, +3.045 | +0.2/−0.4 |
| \|K(997 Hz)\|² from the coefficients | +0.691 dB | the standard's 0.691 |

Ours reads within 0.003 dB of the exact peaks; Annex 2's plain 4× reads up to 0.3 dB under, as a
plain 4× meter does (both within the tolerance).

**The unit:**
- **Null**: bit-exact with no reference (every listen button), bypassed (reference heard), with a
  reference loaded and the mix heard (free, locked, loop), and back on the mix after the fade.
- **Block sizes** 1, 37, 64, 512 and 4 096 (beyond the 512-sample piece): output and every reading
  identical to the bit, through a switch to the reference with the integrated match moving and a
  switch back with the short-term match and an offset. (The first run caught the crest factor's
  sum depending on the pieces; it now sums sample by sample into the step's own sum.)
- **Exact rest**: a twin that skips whenever the unit rests hears and reads the same to the bit,
  and rests 4.2 s of 6 (locked, past the reference's end; listening to the mix or the silent
  reference) and 2.0 s of 6 (free, song stopped).
- **The switch's click**: the output's peak above 2 kHz (4th-order high-pass) around a switch
  between 100 Hz and 150 Hz sines at −6 dBFS: hard switch **−43.2 dBFS**, the 30 ms crossfade
  **−94.0 dBFS** (the 150 Hz sine's own residue through that high-pass is about −96: the fade
  adds next to nothing; unshaped `sin`/`cos` read −66.2).
- **Level through the crossfade**, switching every 100 ms between unrelated noises: −0.007 dB.
- **Match accuracy** on the unit's output, by a separate meter: integrated, a reference 12 LU
  louder and 11 LU quieter: **−0.002 LU** both; short-term, a reference stepping by 6 dB every
  8 s: **+0.061, −0.023, −0.006 LU**; through a passage 30 dB down the gain moved +0.45 dB.
- **Locked**: the reference heard is `song + offset` to the sample and to the bit, through a seek.
- **Declick**: the largest sample-to-sample step through a seek, a stop and a start (locked) is
  0.0112 and through a loop's wraps 0.0109, against the 100 Hz sine's own 0.0065.
- **Blind**: reproducible from the seed, fair (10 000 trials within ±1.5 %), independent; 12/16
  scores p = 2 517/65 536 exactly.

**The analyses against known answers:**
- Pink noise (made exactly in the frequency domain): −0.004 dB/oct in thirds (−0.006 in sixths),
  every band within 0.10 (0.14) dB of the line; white: +3.012 (+3.011) dB/oct, the theory 3.010.
- A 0 dBFS sine reads −3.010 dB in its band; 6 dB louder draws the same curve (≤ 0.001 dB).
- A smooth tilt (−4 … +5 dB) made in the frequency domain is read back as the difference within
  **0.094 dB** in every band 31.5 Hz–16 kHz; confidence over 0.9 above 100 Hz.
- The match suggestion on that tilt: its shape within 0.17 dB of the tilt; Aequalis fits it in 3
  bands, rms 0.04 dB; half the amount is half the curve; the limit holds.
- Correlation and width: mono +1.0000/0.0000, out of phase −1.0000/2.0000, unrelated
  +0.0005/0.9995, a known half +0.503/0.497; per band, 60 Hz mono under 9 kHz out of phase:
  lows +1.0000, highs −1.0000; silence reads nothing.
- Crest: a sine 3.0103 dB, a square 0.0000, one sample in sixteen 12.0412 (silence around them
  not counted). A −20 dBFS sine: PLR 0.004, PSR mean −0.005.
- The histogram of Tech 3342 #1: 177 steps at −20, 171 at −30, the passage between.
- Confidence: 0 under 8 frames; mean 0.86 at 4 s, 0.96 at 21 s (pink noise).

**The offline report** (`analysis.md`, the DI guitar two ways at 44.1 kHz): the "mix" −15.2 LUFS,
crest 11.2 dB, PLR 8.8, nearly mono (width 0.78); the "reference" −8.1 LUFS, crest 3.6, PLR 1.0,
wide (1.18); the tonal difference from −30 dB (20 Hz) through +5 dB (630 Hz) to −34 dB (16 kHz);
the match suggestion fitted in 6 bands, rms 0.11 dB. **The switch on the unit's own output: the
reference heard at −15.11 LUFS against the mix's −15.20 over the same stretch (+0.09 LU)** — the
integrated match levels the *whole played spans* exactly; over a part of them it is off by how
much the two versions' loudness ratio moves within the song (the reference is clipped, the mix
is not).

**CPU** (release, 48 kHz, blocks of 512, best of three, share of one core; measured while two or
three other builders were compiling on the machine, so these are upper bounds):

| Case | One core |
|---|---|
| The reference heard (read, analysed, matched) | 1.61 % |
| The mix heard, the reference analysed beside it | 1.59 % |
| The mix alone (no reference loaded) | 0.91 % |
| At rest (silence skipped, the song stopped) | 0.08 % |
| The tonal worker, both sources, sixth octaves (off the audio thread) | 0.53 %; its longest call 1.45 ms |

The cost is mostly the two 4× true-peak detectors per source (Terminus's, ~100 multiply-adds a
sample a channel) and the band filters; a phase B profile may move the stereo bands to the
worker if it matters.

## Tests, and what each holds

| Test | Holds |
|---|---|
| `tests::the_mix_passes_to_the_bit_when_it_is_heard`, `back_on_the_mix_…` | the null in every configuration |
| `tests::every_block_size_gives_the_same_bits` | block-size independence at tolerance 0, output and readings |
| `tests::rest_is_exact_and_reached` | exact rest, and that it rests |
| `tests::the_crossfade_does_not_click`, `…_keeps_the_level_of_unrelated_signals` | the click and the equal power, measured |
| `tests_match::the_integrated_match_…`, `the_short_term_match_…` | within 0.1 LU on the output; quiet passages held |
| `tests_match::a_locked_reference_…`, `the_reader_declicks_…` | the sync to the sample; no click at any jump |
| `tests_match::the_blind_test_…` | seeded, fair, independent, hidden, exact score |
| `tests_ebu::*` | EBU Tech 3341/3342 cases, BS.1770-4 Annex 2, K at 997 Hz |
| `tests_analysis::*` | the known answers above; the tonal curve the same at every piece size |
| `tests_shared::the_shared_k_weighting_moves_no_bit_…` | Comprimo and Terminus bit-identical to before the sharing |
| `tests/comparatio_alloc.rs` | no allocation after `new`, every path, the worker's too |

`cargo test -p cantus-dsp --no-fail-fast`: 946 passed (17 ignored), and every integration test;
`cargo clippy -p cantus-dsp --all-targets -- -D warnings` clean; `cargo fmt -p cantus-dsp`.
