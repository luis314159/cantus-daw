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
  **−94.0 dBFS** (the 150 Hz sine alone through that high-pass reads −96.2, measured: the fade
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

**CPU** (release, 48 kHz, blocks of 512, best of three, share of one core). Measured twice:
first with two or three other builders compiling (the numbers in brackets), then with no
compiler running at the start (other sessions' `cargo` processes were alive, possibly running
tests, so these are still upper bounds):

| Case | One core |
|---|---|
| The reference heard (read, analysed, matched) | 1.45 % (1.61 %) |
| The mix heard, the reference analysed beside it | 1.34 % (1.59 %) |
| The mix alone (no reference loaded) | 0.69 % (0.91 %) |
| At rest (silence skipped, the song stopped) | 0.056 % (0.08 %) |
| The tonal worker, both sources, sixth octaves (off the audio thread) | 0.41 % (0.53 %); its longest call 1.03 ms (1.45 ms) |

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

## Verified only in emulation, and the misses — honestly

- **No real reference track and no ear has heard it.** Everything above is tests and an offline
  report; the switch's click is measured, not listened to. The owner's ears belong in Phase B
  (`docs/beta-checklist.md` steps: A/B a commercial master against a mix, blind ABX ten trials).
- **The outside sources are definitions, not the EBU's files.** Tech 3341 cases 6 (5.0), 7, 8,
  10, 11, 13, 14 and 20–23 and Tech 3342 cases 5 and 6 are recordings the EBU distributes; they
  were not downloaded (behind the proxy) and are not run. Terminus's own suite holds the same
  meter's 3341/3342 synthetic cases too. Comparatio meters **stereo only** (no 5.1).
- **The true-peak cases are faded in and out** over 100 ms (an abrupt start overshoots between
  its first samples; the EBU files' own start was not checked).
- **The BS.1770-4 Annex 2 coefficients** were typed from the standard's table (Annex 2,
  Table 1); the phases' symmetry (phase 3 is phase 0 reversed, phase 2 is phase 1 reversed)
  and their readings within the tolerance on all five cases agree with that, but they were not
  diffed against a copy of the PDF on this machine.
- **Tests were written beside the code, not each before it.** Several of them caught real
  defects on their first run: the crest factor's sum depending on the pieces, the `f32` glide
  that never landed, the reader's skip leaving another state than reading, the loop's wrap
  inside a fade, the short-term gate raising the edge of a quiet passage, the unshaped fade's
  −66 dB click, and a confidence that called a whole song's balance doubtful because its
  loudness moved.
- **The confidence is a model, not a proof**: a standard error that counts half-overlapped
  frames as `n/2` independent ones. It is right for stationary noise (tested) and indicative on
  music. Its 1 dB "half confidence" point is a choice, said in the code.
- **The integrated match is exact over the whole played spans, not over every part of them**
  (+0.09 LU over the second half of the guitar report): that is what "integrated" means; the
  short-term mode is the one that follows section by section.
- **A jump during a fade** (a seek within 5 ms of another) drops the older fading voice at the
  level it had faded to: the one step left. **A reference swapped while it is heard** starts
  the new one from silence but cuts the old one (the reader cannot read a file it no longer
  has): Phase B's engine fades to the mix before swapping (below).
- **The free playhead runs only while the song plays.** A reference that plays on its own with
  the song stopped (a "reference transport") is Phase B's, if the owner wants it.
- **CPU was measured on a shared machine** (other sessions' builds and tests): upper bounds,
  said where they are given.
- **The match EQ is a suggestion only**: Comparatio never filters the mix. Minimum or linear
  phase is Aequalis's mode when the engine hands it the fitted bands.

## Phase B — the plan

### The engine (`cantus-engine`, `cantus-core`, `cantus-project`)

1. **The kind and the model.** `EffectKind::Comparatio` (the integrator's number), its
   parameters (listen, match, offset, fade, sync with its offset and loop in seconds, the blind
   seed and trial, the reference loudness source, the selected reference) and a schema bump with
   **the reference list in the project**: each reference a path (relative to the project, as an
   audio clip's), a name, its cue point, its loop region and its offset, and which one is
   selected. Every edit a `Command` with its inverse and its undo-fuzz arm. References are never
   copied into the project file.
2. **Decoding off the audio thread.** References load through the existing loader
   (`SampleCache::load_audio(path, rate)`: WAV, AIFF, FLAC, Ogg; resampled to the session's rate
   with its windowed sinc; in the background, silence until it arrives), held as
   `Arc<SampleData>` and handed to the master's unit by the message path the sampler uses (no
   allocation or lock on the audio thread; the old `Arc` dropped on the main thread). **In
   memory or streamed**: five minutes of stereo at 48 kHz is 115 MB in `f32`; with several
   references that argues for streaming the long ones through the engine's `stream_cache`
   (Exemplar's disk streaming). Phase B decides by measurement (load time, and memory against
   the quiet budget of rounds 38–39).
3. **The whole-file analysis at load** (`analyse_whole`, on the loader's thread): the integrated
   loudness for the whole-file match, and the reference's curve, histogram and readings for the
   panel before a note has played.
4. **The master's slot.** The unit sits **after the master's chain, before the master meter**
   (the user compares what the master sends out), as a fixed post-chain slot rather than an
   ordinary insert, so the reference is never processed by the master's own effects. The export
   **never** renders it (an export with the reference selected renders the mix; a test holds
   it). Its transport is the engine's `SongPosition` reduced to `modfx_clock::Transport`, as for
   the modulation family.
5. **A reference swap** while the reference is heard: the engine first sets the mix, waits for
   the meter's `Heard::Mix`, then swaps (no click; the swap test).
6. **Telemetry.** `process_tapped` writes the four streams into a lock-free ring that the UI
   thread drains into two `Tonal` analysers (the worker); `meter()` is published every 100 ms
   step through the telemetry path the other units use. Blind mode publishes `Hidden`.
7. **Tests:** the engine's null at the defaults (`fx_comparatio.rs`); the table-driven
   `fx_contract.rs` (block sizes, split invariance, exact rest twin); offline equals real time
   with the reference heard; `rt_memory.rs` with a reference playing and one loading while the
   song plays; and **the end-to-end test** through `CantusApp::assemble`: load a reference (a
   WAV the test writes), play a track, press the reference button, hear the reference on the
   master at the mix's integrated loudness within 0.1 LU, press the mix button, hear the mix to
   the bit.

### The panel — a design sheet first, for the owner's approval

- **A personality of its own**, distinct from every unit: a measuring instrument — two matched
  channels side by side, **fine strokes** (hairlines, a measuring scale), no pedal-like knobs.
  Its centre is a **dark display in both themes** (Slate and Paper), like an instrument's
  screen, showing:
  - the **two tonal-balance curves** (the mix and the reference, in two kit accents with a
    legend naming them) and **their difference** below on its own zero line, the uncertainty
    drawn as a soft band and the curve fading where the confidence is low; a third- and
    sixth-octave switch; the match-EQ proposal drawn over the difference, with its amount and
    limit as two plain sliders and "Send to Aequalis" as a button;
  - the **loudness**: integrated, short-term and momentary for both, side by side; the match
    gain in dB; the mode (Integrated, Short-term, Manual) and the offset;
  - the **dynamics**: crest factor, peak to loudness, peak to short-term loudness, loudness
    range, and the two short-term histograms mirrored on one axis;
  - the **stereo**: correlation and width for the whole, the lows, the mids and the highs;
  - the **true peak** of each.
- **Big A/B buttons** ("Your mix" and "Reference", the reference's name under it) and a
  **Blind** button that opens the test (A, B, X; "X is your mix" / "X is the reference"; the
  trials; the score and the chance of guessing it at the end; the preference test beside it).
  Every label whole, **nothing abbreviated in English or Spanish** ("Loudness range", not
  "LRA"; "Rango de sonoridad").
- **The reference list**: a strip of the loaded references (drop a file, or the browser's "Use
  as reference"), each with its name, length, integrated loudness and an overview where the cue
  and the loop region are dragged; the sync (Free, Locked to the song, Loop) with the offset in
  bars and beats and in seconds.
- Shot by its own `shoot` example at 75, 100, 150 and 200 %, Slate and Paper, English and
  Spanish, at rest and playing; beside the reference products' windows (kept under
  `C:\projects\daw-fx\refs\`, never committed); the frame budget measured with it open; then
  **stop for the owner's approval**.

### Presets

The defaults are the null (the mix heard); a starting preset; and factory presets that are ways
of comparing: "Mastering: integrated match", "Mixing: live short-term match", "Loop the chorus",
"Blind check, ten trials", "Low end check" (sixth octaves, the lows' stereo), "Loud master: offset
−1 dB" — named in both languages, ten or more.

### CLAP/VST3 export, and loading a reference in another host

- The exported plug-in is the same `FxHost` editor, bit-identical to the engine at the null.
- **Loading a reference inside another host**: the editor's own "Load reference…" (a native
  file dialog on the plug-in's UI thread) and drag-and-drop onto the editor; decoding and
  resampling on a plug-in background thread (at the host's rate from `activate`); the paths
  saved in the plug-in's state (CLAP `state`, VST3 `getState`) with each file's size and hash,
  and **a missing file reported, never silently skipped**; the audio itself is *not* saved in
  the state (sessions would grow by hundreds of megabytes). Locked sync reads the host's
  transport (CLAP `transport`, VST3 `ProcessContext`).
- The CLAP/VST3 tests over every descriptor, plus one that loads a WAV through the state.

### Docs

`docs/plugins/comparatio.md`, the manual's section in English and Spanish, the AI tools' unit
description, the site's unit card, its ADR (the master's post-chain slot, references in the
project by path, never exported), and `docs/beta-checklist.md`'s listening steps.

## Left

- Nothing in the DSP phase that does not need the owner: his **approval of the panel's design
  sheet** (Phase B's first step) and his **ears** on a real A/B and ABX.
- For the integrator: one `lib.rs` line; `comprimo_loudness.rs` now uses Terminus's
  K-weighting (guarded by `comparatio_tests_shared.rs`); `terminus_loudness.rs` gains an
  additive `LoudnessMeter::reset`. No kind, no schema and no ADR number taken.

## Phase B (3), to the finish (2026-11-17): sheet v1 approved

Branch `fx/wave-c` (worktree `C:\projects\daw-fx\wave-a`), from cae94d52. The owner approved
sheet v1 as an **ordinary insert** meant for the master's last slot (renders never hear the
reference), and step 3 as proposed: a list of references, the match EQ sent to Aequalis,
loading in the plug-in.

- **The list of references** (`EffectSpec::refs`, `fx::comparatio::{References, Reference}`):
  at most eight, each a path with the **size and hash** it had when loaded, and the Aequalis
  its match EQ went to; the one compared stays `EffectSpec::ir` (the engine's path
  unchanged). `Command::SetReferences` sets the list and the one compared together, one step,
  its inverse the pair it replaced; refused for a path twice or empty, a ninth, one compared
  that the list lacks, another kind; repaired on load (`repair_project`); **its own undo-fuzz
  stream** (`crng`, applied and refused counted). **Still schema 66** (this branch's own):
  `migrate_65_to_66`'s note extended, `fixtures/v66.cantus` regenerated (a list of three, the
  one compared with its print, the match EQ's amount 0.75), its test extended.
- **Each keeps its own analysis**: the store reads every listed reference once and keeps its
  `Summary` (length, whole-file loudness and readings, the middle's tonal curves in thirds and
  sixths, a 240-peak overview, the print) after its audio is let go.
- **The print** (`comparatio_store::fingerprint`): size + FNV-1a of the size, the first and
  last 64 KiB and 62 blocks of 4 KiB between (milliseconds at any length); one byte changed in
  the middle changes it (tested). The strip says **missing** or **changed since it was
  loaded** in red; loading the file again takes its new print.
- **The editor finished**: the reference strip's numbered memory keys (the one compared lit
  in the reference's voice), *Load reference…* (native dialog), *Remove*, audio files dropped
  on the console (taken from the window's own import), the name line with length and
  loudness, the overview **dragged into a loop** (`sync` 2, `lstart`, `lend`, one step) and the
  loop drawn on it. **Your mix's tonal curve is live** (`picture::Live`: the slot's analyser
  tap, the pair's first — the input's middle — measured on the editor's thread in thirds and
  sixths; restarted with the unit's measurement); the reference's is the same middle of its
  whole file, so like meets like. Curves remade only when something they come from changed.
  The demo feeds the same `Live`.
- **The match EQ**: the suggestion drawn over the difference in orange (`MATCH`, apart from
  every colourway's voices) with its legend, before anything is sent; the *Match EQ* card
  (`eqamt` 0 … 1 shown in %, default 0.5; `eqmax` 1 … 12 dB, default 6 — the DSP's
  defaults; display keys, no sample changes). **Send to Aequalis** fits it (`aequalis::fit`,
  at most 6 bands, minimum phase) and inserts an Aequalis **just before the unit**, its output
  at the suggestion's makeup (`Batch` of `InsertEffect` + `SetReferences` keeping its id: one
  step); a second send **replaces that Aequalis's spec in place** (`ReplaceEffect`, same id,
  one step). A user's own EQ is never touched; the key shows only where the host has a chain
  (Cantus).
- **learn_everywhere green for Comparatio**: `listen` offers automation and learn on the
  selector (0 ↔ 1), the two lamps and the A, B, X keys (`stepped_offer_one`, each its value);
  `seed` and `trial` excused by name (the blind test's order and trial, kept by its keys: a
  lane or a controller would give the answer away), as Loquax's object is. **A new blind
  test's seed comes from the clock** (never seed + 1: after a preset or a reload put it back
  to 0, an order could have been learnt).
- **Presets** fourteen (`reference.toml`, roles master/bus, styles across the library): the
  whole file's loudness, section by section, by hand, the reference 1 dB down, locked to the
  song, a chorus loop (45–75 s), the first thirty seconds, quick (10 ms) and smooth (150 ms)
  switching, low end in detail, a gentle and a full match EQ, an album locked by its whole
  file, a worship mix against a live record. None names `listen`, `seed` or `trial` (loading
  one returns to your mix). `preset_tool fit` has nothing to set: Comparatio has no output
  key and never changes the mix's level (the audition renders the mix). The audit's
  Comparatio line removed (floor 12).
- **Export** `Cantus Comparatio` (CLAP and VST3; class id by its rule `8C4B6490…`; features
  `analyzer`, `utility`; `Fx|Analyzer`): the same editor; the state is the project's format
  (`refs` and `ir` beside the values, never audio; `state::with_references`,
  `load_references` repairing as a project loads); `comparatio_post::RefPost` keeps the list
  on the main thread, asks the store to read the one compared on its own thread at the
  processor's rate (at activation, or when the editor or a state names it), posts the file to
  the processor as Fractus's lanes are posted, and lets go of the others' audio
  (`keep_only`); the memory builder answers the unit's need by **waiting on its own thread**
  (`comparatio_store::wait_for`, giving up when the read fails or the processor goes). **A
  host's offline render hears the mix**: CLAP's `render` extension (offered by Comparatio
  alone) and VST3's `setupProcessing` `processMode == kOffline` set the processor's `listen`
  to the mix before the unit is configured. Ids appended and blessed.
- **Tabula**: never a pedal (decided at the contract; now with its reason in full — the render
  guarantee strips Comparatio *slots*, a pedal inside a board would sit outside it) and held
  by `fx::comparatio::tests::it_is_never_a_pedal`.
- **AI words** (`tools_mixer.rs`'s effect description; `agent_units` gains `eqamt`, `eqmax`
  and the list). **Site card** (`web/src/units_data.rs`, EFFECTS 39; big 1800 × 1157, thumb
  640 × 411). **Docs**: ADR 0146 phase B, `docs/plugins/comparatio.md`, the manual EN/ES
  (after Purus).

### Measured

| What | Measured |
|---|---|
| e2e (`comparatio_e2e`, through `CantusApp::assemble`): the reference heard on the master against the mix, both steady noise, the file 17.1 dB louder (−13.80 against −30.92 LUFS) | **−0.023 LU** (−30.94 heard) |
| e2e: a render with `listen` reference, X, One, Two | the dry master **to the bit** (each) |
| e2e: live in odd blocks (256, 333, 512, 17, 1 024) with the mix heard, 3 s | the render **to the bit** |
| e2e: back to the mix, from 100 ms after the switch | the render **to the bit** |
| Two references in a list, 20 dB apart in their files | their summaries 19.3 dB apart (K-weighting's tilt between 300 Hz and 1 kHz), each curve's loudest band at its own tone |
| The print of a 3 s WAV; one byte changed in the middle | equal to the file's size; the hash changes |
| Pointer (`comparatio_drag`, real events through the window the app draws) | 4 tests: six selector/lamp/key presses, two numbered keys, Remove, Load reference…, a drop, a loop dragged (3.0 → 9.0 s ± 0.4 on the 12 s demo), Send to Aequalis inserted then updated in place — each one undo step |

### Shared files touched (marked `fxc:comparatio`)

Core: `effect.rs` (`EffectSpec::refs`), `command.rs` (`SetReferences`), `freeze_guard.rs`,
`record_mode.rs`; tests `undo_invariants.rs` (its own stream). Project: `format.rs`
(repair, the note, the fixture test), `examples/write_fixture.rs`, `fixtures/v66.cantus`.
UI: `views/fx/host.rs` (five `FxHost` methods with defaults, `DawHost`'s),
`views/eq_editor.rs` (`band_fields`, pub(crate)), `kit/skin/comparator.rs` (`MATCH`),
`agent_units.rs`; tests `learn_everywhere.rs`, `preset_audit.rs`. Plug-ins: `Cargo.toml`
(feature, `hound` for tests), `build.rs`, `src/{desc,lib,instance,memory,state,editor}.rs`,
`src/clap/ext.rs`, `src/vst3/plugin.rs`, `tests/golden/ids.txt`. Engine:
`fx/comparatio_store.rs` (its own). AI: `tools_mixer.rs`; golden `mcp-tools-list.json`.
xtask: `main.rs`. Web: `units_data.rs`.

### Design sheets

`C:\projects\daw-fx\compartir\comparatio\design\v2-step3\` (23 PNGs and its `index.md`), shot
by `shoot_fxc` with the clip-aware probe: no cut word. Looked at: Slate English (start, the
full match EQ), Paper Spanish (reference, loop, full match EQ), the ivory colourway. One fix
came from looking: the orange curve crossed *More in your mix*; the which-way words now sit
on dark plates. The loop's band was made stronger.

### Suites (Windows, 2026-11-17, one crate at a time; clippy `-D warnings --all-targets` on each touched crate)

cantus-core 815 (+5: the list's validity, the shown list, the repair, never a pedal; the fuzz
counts Comparatio's references applied and refused), cantus-dsp 1 097 + Chorda model 2's
CPU test (red with other builders compiling, green re-run alone — untouched here),
cantus-engine 1 306 + `sync_midi::the_out_thread_sends_on_time` (the MIDI out thread's 90th
percentile 15.5 ms with the machine loaded, red twice; untouched — the engine's only change
is `comparatio_store.rs`), cantus-project 194, cantus-plugins 94 (+3: Comparatio's state,
heard and offline, a missing file), cantus-ai 51, cantus-ui 2 731 (+ `preset_render`'s known red: Arca's
three leads' CPU under load, +0.23 … +0.28, green re-run). Clippy clean on core, project,
engine, plug-ins (default and `--no-default-features`), ai, ui; `cargo fmt -p` on each.
Not run: the Linux check (WSL), the whole workspace at once.

### Verified only in emulation

- No ear has compared a real record in the app: the e2e and the pointer tests drive the
  real engine and window with generated noise and the demo's music.
- The plug-in in our own harness only (state, processor, memory builder, offline flag set
  directly): not yet in a third-party DAW, so CLAP's `render` call and VST3's `processMode`
  as real hosts send them are unverified; the native file dialog inside a host's window is
  not exercised by a test (`pick_for_tests` stands in for it in Cantus's).
- **A drop onto the plug-in's window is not taken**: egui-baseview 0.7 swallows baseview's
  drag-and-drop events (it answers them as handled without passing them to egui, and its
  window constructor is private, so it cannot be wrapped). In Cantus a drop works (tested).
- The mix's live tonal curve measures the analyser tap's middle `(L + R) / 2`; a mix whose
  sides carry what its middle does not is compared on its middle (the reference likewise).

### Left

- Owner-only: the v2-step3 sheets; listening (A/B a commercial master against a mix, a blind
  ABX of ten trials — `docs/beta-checklist.md` at the integration); the choices in the
  sheet's `index.md` (per-reference settings, how many references and how many kept in
  memory, the match EQ's colour and defaults, patching egui-baseview for a drop onto the
  plug-in).
- At the integration: the site's pictures (`web/assets/units/comparatio-*.webp`, big
  1800 × 1157, thumb 640 × 411); renumbering ADR 0146 and schema 66.

## The owner's answers (2026-11-17), built on the same branch

1. **Each reference keeps its own playback** (`Reference::playback`: `sync`, `soff`,
   `lstart`, `lend`). The unit's keys stay the live values, so automation, the engine, the
   plug-in's parameters and every older path are unchanged; comparing another reference
   (`fx::comparatio::switch_to`, `switch_command`) saves the unit's playback in the outgoing
   entry and brings the newcomer's own back — its defaults the first time — as one `Batch`
   (`SetReferences` + `SetEffectParam`s), undone exactly. **Global on purpose**: the match
   (`match`, `offset`, `refl`), the crossfade and the display are *how* you compare, and a
   comparison that changed its own rules under you as you switched would not be a fair one;
   *where* a reference plays (its chorus, its offset against your song) is the reference's.
   A playback out of range is refused by `SetReferences` (`References::valid`) and clamped on
   load. The undo fuzz draws switches as the editor makes them (applied and refused). Still
   schema 66 (`migrate_65_to_66`'s note; `fixtures/v66.cantus` regenerated: one reference
   holds a stored loop 45–75 s; v64 and older load). The plug-in's state carries it (the same
   JSON); the plug-in's editor writes the newcomer's playback onto the host's parameters.
2. **A drop onto the exported plug-in's window**: egui-baseview 0.7.2 vendored in
   `third_party/egui-baseview` (first commit 5b068553 unchanged, then the patch marked
   `CANTUS PATCH`), wired by `[patch.crates-io]` as cpal and jack-sys are. Its new
   `drop::input` turns baseview's `DragEntered`/`DragMoved`/`DragLeft`/`DragDropped` into
   egui's hovered and dropped files and the pointer, and answers the system's drop target
   `AcceptDrop(Copy)`; `EguiWindow::on_event` calls it first. The editor takes the drop as
   it does in Cantus. `THIRD-PARTY-NOTICES.md` has its own section (MIT OR Apache-2.0; the
   change named, Apache-2.0 §4(b)); `third_party/README.md` says what changed and why.
   **Only a real host can confirm** that its own window lets the system's drop reach ours
   (some hosts wrap plug-in windows in their own drop targets) — a beta-checklist step.
3. **Only the one compared is in memory**: the store keeps no spare (`SPARE` = 0; a track
   made in memory by a test is never let go). Every listed reference is read once for its
   summary and let go; comparing one reads it again from disk on its thread and **adopts
   its summary** (no second analysis while its print is unchanged; a changed file is
   analysed again). **While it is read you hear your mix** — the snapshot names no
   reference until it is in (the store's rule since step 1), and the strip says *Being read
   from disk: your mix plays meanwhile* — never silence and never the previous reference
   passing for the new one; it fades in from your mix when it arrives, as any switch.

| Measured | |
|---|---|
| Eight 2 s references in a list (`eight_references_hold_the_audio_of_one`) | **768 000 bytes** of audio held — exactly one reference's; the fifth compared: read again, its summary the same `Arc` (not analysed again), the first let go |
| `rt_memory::comparatio_never_allocates`, new phases: another reference being read (the snapshot naming none), then arriving and fading in | nothing allocated or freed on the audio thread |
| Pointer: a loop dragged on the first reference, the second compared, the first again (`each_reference_keeps_its_own_loop`) | the second at its defaults, the first's loop back to the tenth of a second; each switch one undo step |
| The drop through the plug-in's window handler (`a_file_dropped_on_the_plug_in_s_window_is_compared`, a 1.5 scale) | `AcceptDrop(Copy)`, the pointer in logical points, the file listed, compared and its print kept |
| The plug-in's reference heard (now matched by its whole file, so the read's timing is out of the test) | −23.79 against −23.82 LUFS (0.03 LU) |
