# The effects side round, wave B — `cedo`: Cedo, the ducker (DSP phase)

This is branch `fx/cedo`, made from `fx/integration` d3b51a09. It is **DSP only**: there is no
`EffectKind`, no core parameters, no engine unit, no panel, no schema bump and no ADR number. The
name is provisional (`cantus_dsp::cedo::NAME`). Latin *cedo*: "I make way".

**Phase A-2** (after the integrator's review) closed four of the first phase's misses: a sharper
linear-phase kernel, a fade when the song starts or seeks inside a duck, an RMS detector, and MIDI
velocity on the depth. Its before/after is in the section *Phase A-2* below; the rest of this file
describes the unit as it is now.

Cedo makes room for something else. It ducks a track by:
- the **audio of another strip** (a kick, a voice), through a filtered key, a threshold, and an attack, hold and release;
- or, with **no audio at all**, a **drawn shape** fired on every beat division of the song or on the notes of another track.

Its references for *function* (inspired by, never copied) are:
- the sidechain compression and "LFO tool" ducking that FL Studio users do with Fruity Limiter, Peak Controller and LFOTool;
- Nicky Romero's Kickstart, Cableguys' VolumeShaper;
- broadcast voice-over duckers.

## Done

| Layer | Files (owned) | Shared files |
|---|---|---|
| DSP | `cantus-dsp/src/cedo.rs`: the unit, its settings, meter, latency, the classic pump. `cedo_run.rs`: process, skip, rest, reset. `cedo_env.rs`: the key filter and the envelope. `cedo_shape.rs`: the pump. `cedo_band.rs`: the band split. `cedo_testkit.rs`: the automation shapes mirrored, a driver that splits blocks at events, the exact grid, signals and measures. Tests: `cedo_tests.rs`, `cedo_tests_measure.rs`, `cedo_tests_shared.rs`, `cantus-dsp/tests/cedo_alloc.rs`. Example: `cantus-dsp/examples/cedo_sounds.rs`. | `cantus-dsp/src/lib.rs`: one line, `pub mod cedo; // fx-b:cedo`. `comprimo.rs`: the key band's section loop taken out of `Comprimo::filter_key` into `pub fn key_band`, and its `Delay` made public. Both are marked `fx-b:cedo`, and Comprimo's outputs are bit-identical (below). |
| Docs | this file | — |

### What is shared with the family, and how it is guarded

- **Comprimo's key path** (`comprimo::key_band`). The key band's sections are Aequalis's `design` run in `f64`, and a band that hears silence and has died away is stopped whole. The body is moved from `Comprimo::filter_key`, with the same operations in the same order. Cedo's key filter is a 24 dB/oct high-pass or a band-pass with its Q, through the same function.
- **Comprimo's stereo delay line** (`comprimo::Delay`): Cedo's lookahead, its dry path, its key line and its amount line.
- **Lapsus's curve** (`lapsus::Curve`), unchanged: Fluctus's points with the automation editor's segment shapes and tensions, sorted without allocating. The engine will pass `fluctus::shape_at`, as for Lapsus and Cribrum, so a shape bends exactly as in an automation clip.
- **The family's clock** (`modfx_clock::Clock`, `Transport`, `DIVISIONS`): the song sample, the jumps and the playing state, as Lapsus and Cribrum read them. **Swing** follows the LFO's convention (`modfx_lfo`): 0 straight, 1 = MPC's 75 %.
- **The family's glide** (`modfx_line::Glide`): depth, mix and output follow a change over 10 ms and land exactly.
- **The sidechain convention** of Comprimo, Cribrum and Aequalis: `process(left, right, key: Option<(&[f32], &[f32])>)`.
- **Guarded:** `cedo_tests_shared.rs` hashes Comprimo's output and its meter for five settings:
  - the defaults;
  - six key EQ bands (cut, bell and band-pass) with the high-pass and the key gain, on its own input and on an external key;
  - Opto at 2× with a partial link;
  - Mastering at 4× with 20 ms of lookahead.

  The hashes were taken **before** the sharing (commit 4929d9fe), and the test passes after it. Comprimo's own tests are green in the full suite.
- **Not shared, and why:**
  - Comprimo's *detector* is its gain computer: a ratio, a knee, styles and feedback. A ducker's trigger is a threshold firing an envelope of exact lengths, so they share the key filter, not the ballistics.
  - `dynfx_follow` smooths a level. Cedo's hold must count from the key's last sample over the threshold, to the sample, which a follower's release would blur.
  - Lapsus's `bound` is one private line, and the swung grid is not Lapsus's. Cedo writes the same expression (`round(j·b·60/tempo·fs)`) with the swing in it.

## The DSP API

```rust
use cantus_dsp::cedo::{Cedo, CedoSettings, Trigger, Listen, CLASSIC_PUMP, latency_for};
use cantus_dsp::cedo::band::{Band, Crossover};
use cantus_dsp::cedo::env::{KeyFilterMode, ReleaseShape};

let mut c = Cedo::new(sample_rate, shape_at);  // allocates here only; shape_at = the automation shapes
c.set_shape(&CLASSIC_PUMP);                    // the drawn shape, y = the amount (0 … 1), ≤ 64 points
c.set(&settings);                              // between blocks; depth, mix and output glide 10 ms
c.set_transport(&transport);                   // each pass (Tempo; MIDI reads the tempo)
c.note(offset, velocity);                      // a note-on from the trigger track (MIDI)
c.process(&mut left, &mut right, key);         // key: the sidechain from another strip
c.latency();                                   // lookahead + the linear-phase kernel's delay
latency_for(&settings, sample_rate);           // the same without a unit
c.meter();                                     // CedoMeter (below)
c.response_db(hz, amount);                     // the band split's magnitude at an amount (the display)
c.at_rest(); c.rest_blocker(); c.skip(n); c.reset();
```

**`CedoSettings`** (all in real units; `Default` is the null, with a depth of 0):

| Field | Range | Default | Meaning |
|---|---|---|---|
| `trigger` | `Sidechain`, `Tempo`, `Midi` | Sidechain | What fires the duck. |
| `depth_db` | 0 … 80 (∞) | **0** | How far it ducks; 80 or more is silence. 0 is the null. |
| `threshold_db` | −90 … +6 | −24 | The filtered key's sample peak that fires it. |
| `attack_ms` | 0 … 500 | 1 | To the full depth along an S (`3p² − 2p³`), exactly this long. |
| `hold_ms` | 0 … 2 000 | 40 | Kept at full depth this long after the key's **last** sample over the threshold. |
| `release_ms` | 0 … 5 000 | 150 | Back to nothing, exactly this long. |
| `release_shape` | `Linear`, `Exponential`, `SCurve` | Exponential | `p`; `(1 − e^{−5p})/(1 − e^{−5})`; `3p² − 2p³`. |
| `key_filter` | `Off`, `HighPass`, `BandPass` | Off | 24 dB/oct high-pass, or a band-pass (12 dB/oct sides). |
| `detector` | `Peak`, `Rms` | Peak | The key's sample peak, or its RMS over `rms_ms` (the family's follower on the square). |
| `rms_ms` | 1 … 300 | 10 | The RMS window. |
| `key_hz`, `key_q` | 20 … 15 000; 0.3 … 12 | 100; 1 | The key filter's frequency, and the band-pass's Q. |
| `lookahead_ms` | 0 … 10 | 0 | The audio waits for the detector; the latency. |
| `division` | index into `DIVISIONS` (8/1 … 1/32, dotted, triplets) | 1/4 | The shape's cycle. |
| `swing` | 0 … 1 | 0 | Every second cycle late, by up to half a cycle (MPC's 75 %). |
| `edge_ms` | 0 … 10 | 1 | The shape's edges smoothed by a centred Hann kernel this wide. |
| `band` | `Whole`, `Lows`, `Highs` | Whole | Where the duck acts. |
| `crossover` | `MinimumPhase`, `LinearPhase` | Minimum phase | How the band is split. |
| `cross_hz` | 40 … 2 000 (linear phase from 60) | 120 | The crossover. |
| `link` | 0 … 1 | 1 | 0: each channel ducks by its own key channel; 1: both by the louder. |
| `mix` | 0 … 1 | 1 | How much of the duck is applied. |
| `out_db` | −60 … +24 | 0 | The output level. |
| `listen` | `Output`, `Delta`, `Key` | Output | Delta: what the duck takes away. Key: the filtered key, aligned to the output. |
| `velocity` | on / off | off | MIDI: a note's velocity scales the depth in decibels (127 is the whole depth, 64 about half of it). |

**The gain law.** Both triggers give an amount `a` (0 … 1) per channel, and the band gain is
`G = 1 − a·mix·(1 − 10^(−depth/20))`. It is linear in the amount, so the shape drawn is the level
heard. Exactly 0 is skipped, never multiplied.

**`CedoMeter`** holds, for the last block:
- the most reduction (dB) and each channel's reduction at its end;
- the amount;
- the filtered key's peak (dBFS) and whether it reached the threshold;
- **where the shape's cycle is** (0 … 1, or −1): the display's playhead;
- the input and output peaks.

## How it works (each file's header says it in full)

**The sidechain** (`cedo_env.rs`):
- The key goes through its filter. Each channel's sample peak, or its RMS (a one-pole of `rms_ms` on the square, through the follower in `dynfx_follow`), is compared with the threshold.
- A sample at or above it **triggers**, and the **hold** counts from the *last* such sample. A kick's low half-cycles that dip under the threshold therefore do not let go while the hold covers them.
- The envelope attacks along an S from where it is, holds, then releases along its shape over exactly the release. All of these are counted per sample, never accumulated.
- A trigger during the release attacks again from where the release has got to.

**Lookahead and alignment.** The audio is delayed by the lookahead plus the linear-phase kernel, and the detector is not. The sidechain's amount is delayed by the kernel only, so its lead over the audio is exactly the lookahead. The pump reads the song position of the audio as it leaves (`song − latency`), and a note's shape starts at the note's own audio.

**The pump** (`cedo_shape.rs`):
- **Tempo.** Cycle `j` starts on song sample `round(x_j·len)`, where:
  - `len` is one division at the tempo;
  - `x_j = j` for even `j` and `j + swing/2` for odd `j`.

  The phase is `(s − start)/(next − start)`, so **the shape's first point is on the grid's sample**. The amount is a function of the song sample alone. It plays while the transport plays; on stop it lets go through its edge.
- **MIDI.** A note plays the shape once from its sample, one cycle of the division long. A newer note starts it again, and after the cycle nothing is ducked.
- **The edge.** A Hann kernel centred on the sample turns a vertical edge into a raised-cosine ramp whose middle is the edge itself:
  - It adds no latency, because the song's future is known.
  - A note's ramp is centred too when the unit's latency covers half the edge, so a note's values already made are made again when it arrives.
  - Where the whole kernel sees one value, that value passes exactly, with no sum. A flat 0 therefore ducks nothing, to the bit.
- **The ring the kernel reads** is filled afresh from the shape after every start, seek, wrap, tempo change, latency change and skip. It never depends on what was processed before.

**The band split** (`cedo_band.rs`), each with an **exact null path**: at a band gain of exactly 1 the delayed input passes to the bit, while the filters run beside it so a duck finds them warm.
- **Minimum phase** (no latency): two Simper SVF sections at the crossover (`k = √2`), whose coefficients are fixed by the frequency alone.
  - They are read as a **shelf whose depth moves**: `(s² + kAs + A²)/(s² + ks + 1)` per section, with `A² = √G`. The highs use the mirrored high shelf.
  - Per section, `|H|² = (A⁴ + ω⁴)/(1 + ω⁴)`. This is monotone from `G` to 1, **with no bump**, and −6 dB at the crossover for a deep duck.
  - Only the outputs' weights move with the gain, never the filters' coefficients. A duck that moves every sample cannot destabilise them.
- **Linear phase:** the lows are a **sharpened B-spline**, `L = S(B)`:
  - The base `B` is two boxcars of `M = 0.44295·fs/fc` (`sinc²`, `B(fc) = ½`).
  - `S(b) = 10b³ − 15b⁴ + 6b⁵` (smootherstep; Kaiser and Hamming's filter sharpening, 1977). It is the base applied three, four and five times (ten boxcars), the cube and the fourth power delayed to the fifth's centre and summed.
  - `S` is non-decreasing from 0 to 1 on `[0, 1]`, and `B` stays in `[0, 1]`, so **`0 ≤ L ≤ 1` at every frequency**: neither the lows' duck nor the highs' can rise above the input. `L(fc) = ½` (−6 dB); the passband is 0.9994 at `fc/4` and 0.973 at `fc/2`; above `2fc` the stopband is under −60 dB.
  - The highs are the delayed input minus the lows, so the bands sum to the delayed input exactly.
  - Everything runs in **integers** (steps of 2⁻³⁶; each boxcar's sum exact and its output a pure function of the sum; the polynomial a sum of integers), so nothing drifts and rest is exact. Per sample it costs ten integer boxcars and two short rings per channel, whatever the crossover or the rate.
  - **The latency is fixed** at the 60 Hz kernel's, `5·(M − 1)`: **1 765 samples, 36.8 ms at 48 kHz**. Moving the crossover never moves the latency, because a shorter kernel reads its input later. A crossover under 60 Hz is minimum phase only.
  - A new crossover **rebuilds** the kernel from the dry line's history, without allocating, as if it had always been that length.

**A start or a seek inside a cycle** (`cedo_shape.rs`) fades the shape in over 10 ms (a raised cosine from nothing), from the sample where the transport landed. The audio before the landing is not ducked by the place the song has jumped to.
- A landing **on a cycle's start or on a bar line** (4/4 from the song's start) is not faded. Any render from bar N starts on a bar line, so it still equals the whole render to the bit.
- The fade is a function of where the song landed, so a skip or a refill of the kernel's window gives the same values.
- A tempo change re-anchors the clock but is not a seek: it does not fade.

**Velocity** (MIDI, when switched on): a note's amount is scaled by `k = (1 − 10^(−depth·v/20)) / (1 − 10^(−depth/20))`, with `v = velocity/127`. The full shape then ducks by `depth·v` dB: velocity scales the depth in decibels, not the level. The scale is part of the shape's values, so the edge kernel smooths a change of velocity from one note to the next.

## Measured

Everything below is at 48 kHz. Run it with `cargo test -p cantus-dsp --lib cedo -- --nocapture`.
The tests' bounds sit just under these numbers, so a regression fails.

### The tempo pump on the song's grid

- **Onsets, sample-exact.** The test runs:
  - 60, 120 and 174 BPM;
  - straight, 50 % and 100 % swing;
  - quarters, eighths and sixteenths;
  - each without and with 3 ms of lookahead;
  - in blocks of 37 (so the edges fall anywhere in a block).

  The shape is a hard square (edge 0), and the duck must start on the grid sample computed **in exact rationals** (`round(position · 60 · fs / BPM)`, halves up, `i128`) — not with the DSP's own expression. **3 978 of 3 978** onsets land on their sample, with the latency taken off.
- **A smoothed edge is centred** (2 ms, at 120 BPM on eighths, into and out of the duck, with lookahead 0 and 3 ms). The sample before the grid and the grid's own, and every pair either side, sum to 1 within **2.4·10⁻⁷**. The ramp is 48 samples either side and nothing beyond.
- **Notes:** a note's shape starts on the note's sample, with and without lookahead. A note inside the duck of the one before restarts it, and the duck ends exactly a cycle after the newest note.

### The sidechain against its settings

A key of 0.5 for 2 000 samples, a threshold of −20 dB, the full depth, blocks of one:

| Set (attack / hold / release) | Linear | Exponential | S-curve |
|---|---|---|---|
| 5 / 20 / 50 ms | **5.000 / 20.000 / 50.000 ms** | 5.000 / 20.000 / 50.000 ms | 5.000 / 20.000 / 50.000 ms |
| 0 / 0 / 10 ms, 12.5 / 7 / 120 ms | exact to the sample | exact to the sample | exact to the sample |

- The duck starts on the key's first sample over the threshold. It reaches full depth `attack − 1` samples later, stays full for `hold` after the key's last sample over the threshold, and is back to exactly 1 `release` samples after that.
- Half way through the release, the amount is 0.500 (linear), **0.076** (exponential) and 0.500 (S-curve), as each shape's formula says.

### The lookahead catching a kick's transient

The key is Pulsus's kick (model 2), with a threshold of −30 dB, attack 5 ms, hold 60 ms and release 100 ms, at the full depth:

| Lookahead | The duck's gain at the kick's first sample over the threshold | The kick's first 10 ms, as the ducked track lets it through |
|---|---|---|
| 0 ms | 0.9999 (−0.0 dB): the duck is not there yet | −7.2 dB |
| 2 ms | 0.642 (−3.9 dB) | −16.8 dB |
| **5 ms (= the attack)** | **0, exactly** | **silence: the whole transient is caught** |

### The band split

- **Not ducking, to the bit.** With the pump ducking the lows over the first quarter of each beat, the 54 000 samples between the hits are each the input's own bits, in both crossovers, with the filters running. The null test covers every band and crossover too.
- **Ducking, at a depth of 24 dB, crossover 120 Hz.** Measured with sines through the DSP, against `response_db`, in dB:

  | | 30 Hz | 60 | 120 | 240 | 480 | 1 000 | 5 000 |
  |---|---|---|---|---|---|---|---|
  | Minimum phase, lows | −23.51 | −18.55 | −5.49 | −0.49 | −0.03 | −0.00 | −0.00 |
  | Minimum phase, highs | −0.03 | −0.49 | −5.49 | −18.55 | −23.51 | −23.97 | −24.00 |
  | Linear phase, lows | **−23.92** | −21.06 | −5.51 | −0.00 | −0.00 | −0.00 | −0.00 |
  | Linear phase, highs | −0.00 | **−0.22** | −5.47 | −23.99 | −24.00 | −24.00 | −24.00 |

  Measured and drawn agree within 0.01 dB everywhere. **Nothing rises above 0 dB at any frequency** (asserted < +0.005 dB).

### Clicks at the edges

The splatter is the energy above 4.5 kHz relative to the whole output. The input is a 1 kHz sine (whose own floor reads −155 dB), with depth 24 dB, the pump on eighths at 120 BPM:

| Edge | Square shape (hard steps) | Classic pump |
|---|---|---|
| 0 ms | −53.9 dB (a click) | −56.9 dB |
| 0.5 ms | −77.2 dB | −80.1 dB |
| **1 ms (the default)** | **−97.2 dB** | **−100.1 dB** |
| 2 ms | −115.3 dB | −118.1 dB |
| 5 ms | −138.9 dB | −138.9 dB |

- The sidechain with a 1 ms attack measures **−89.3 dB** with any release shape. A straight attack measured −78 dB; its corners were the cause, and the S attack removed them without moving any timing.

### The contract

- **Null:** the output is the input bit for bit, delayed by exactly `latency()`, with the samples before +0.0. This holds in ten cases:
  - the defaults (depth 0);
  - the sidechain with no key, a silent key, depth 0, or mix 0;
  - the tempo pump stopped, at depth 0, or at mix 0;
  - MIDI with no note, or with notes at depth 0.

  Each case runs again with 5 ms of lookahead, lows minimum phase, highs linear phase with 2 ms of lookahead, and lows linear phase, in blocks of 64 and 37.
- **Block-size independence**, at tolerance 0: blocks of 1, 37, 64 and 512, compared bit for bit, in three runs that each duck more than a tenth of the samples:
  - the sidechain at a partial link, with the linear-phase lows. Its settings move mid-way to a new crossover (the kernel rebuilt), then to minimum-phase highs, then to Delta with a linear release.
  - the tempo pump at 174 BPM, swung. It changes division and edge mid-way, and also seeks, stops, and starts again at 60 BPM.
  - MIDI with notes at odd offsets inside blocks, two of them 55 samples apart, and the lookahead moved mid-way.
- **Render from bar 5 = the whole render**, to the bit, from the latency on. This holds at 120 BPM straight, 174 BPM swung, 60 BPM with full swing (lows, minimum phase), and 128 BPM highs linear phase. The band cases have a second of silence before bar 5: a filter remembers what it heard.
- **Rest:**
  - At rest after **0.08–0.09 s** of silence (tempo lows; MIDI once the shape has played out), **0.39 s** (sidechain, linear-phase lows), **0.89 s** (a Q 4 band-pass key at 60 Hz, ringing down) and **0.97 s** (the RMS detector with a 50 ms window).
  - The twins agree to the bit on music afterwards: processing zeros, skipped, reset, and new.
  - **The tempo pump rests while the transport plays**: 60 000+ samples of silence are skipped mid-song and the output equals the unskipped render.
  - A new unit is at rest, and silence keeps it there.
- **No allocation after `new`**, under a counting allocator: 0 allocations. This covers:
  - six settings twice, in blocks of 1, 37, 64, 512 and 1 024, with and without a key;
  - the shape replaced, a kernel rebuild, notes, and the transport seeking and stopping;
  - `meter`, `at_rest`, `rest_blocker`, `latency`, `response_db`, `skip` and `reset`.
- **Deterministic maths:** only `crate::math` (clippy's `disallowed-methods`), with `sqrt` IEEE and integer boxcars.
- **CPU** (one core, 48 kHz, a minute in blocks of 256, best of three, `--release`):

  | Setting | Share of a core |
  |---|---|
  | Sidechain, whole | **0.17 %** |
  | Sidechain, band-pass key, minimum-phase lows, lookahead, unlinked | 0.26 % |
  | Sidechain, linear-phase lows | 0.32 % |
  | Tempo, classic pump, edge 2 ms | **0.30 %** |
  | Tempo, edge 10 ms, linear-phase lows (the heaviest) | 1.02 % |

  The machine was a Ryzen 7 7735HS. No other `cargo` or `rustc` ran before or after the measure, and the CPU load was 24 % (desktop applications). The figures include the test driver's copies.

  After A-2 the machine was not quiet again (other builders compiled for over half an hour). Measured beside six `rustc` at 100 % load: sidechain whole 0.33 %, minimum-phase lows 0.49 %, **linear-phase lows 0.78 %**, tempo 0.48 %, the heaviest case 1.93 %. These are about twice the quiet figures throughout (0.33 against 0.17 on the path A-2 did not touch). The quiet measure is for the integrator to repeat: `cargo test -p cantus-dsp --release --lib cedo_cpu -- --ignored --nocapture`.

### Tests and what each holds

- **`cedo_tests.rs`:**
  - `no_depth_no_mix_or_no_trigger_is_a_null_to_the_bit_delayed_by_the_latency`
  - `the_output_does_not_depend_on_the_block_size`
  - `a_render_from_bar_n_equals_the_whole_render`
  - `it_comes_to_rest_exactly_and_its_twins_agree`
  - `the_tempo_pump_rests_while_playing_and_a_skip_keeps_its_place`
  - `a_new_unit_is_at_rest_and_silence_keeps_it_there`
  - `a_shape_with_a_flat_zero_ducks_nothing_and_a_full_one_ducks_to_the_floor`
- **`cedo_tests_measure.rs`:**
  - `the_pump_lands_on_the_song_grid_to_the_sample`
  - `a_smoothed_edge_is_centred_on_the_grid`
  - `the_sidechain_attack_hold_and_release_match_their_settings`
  - `the_lookahead_catches_a_kicks_transient_fully`
  - `the_band_split_is_flat_when_not_ducking_and_never_rises_when_ducking`
  - `the_shapes_edges_splatter_little`
  - `notes_fire_the_shape_on_their_sample`
  - `a_trace_reads_the_reduction_on_each_sample`
  - `cedo_cpu` (`#[ignore]`; run it alone, in release)
- **`cedo_tests_a2.rs`:** the five tests of phase A-2 (in its section).
- **`cedo_tests_shared.rs`:** `comprimos_outputs_are_bit_identical_to_before_the_sharing`.
- **`tests/cedo_alloc.rs`:** `cedo_allocates_nothing_after_new`.
- **Each test was seen failing** where that means something:
  - The hash test failed until the hashes were recorded.
  - A pump that read the song position on a two-sample grid failed both the block-size test and the render-from-bar-5 test.
  - The splatter test failed until its own sine generator was fixed: its `f32` phase was the −78 dB floor.

**The suite:**
- `cargo test -p cantus-dsp --no-fail-fast` passes 941 in the library (18 ignored) plus the integration tests (after A-2).
- `cargo clippy -p cantus-dsp --all-targets -- -D warnings` is clean.
- `cargo fmt -p cantus-dsp` is applied.

## Shown to the owner

`C:\projects\daw-fx\compartir\cedo\sound\` holds 17 WAV files, the `README.md` (what to listen for) and the `render-log.txt` (every setting and number):
- **Kick and pad** (124 BPM, Pulsus's kick):
  - the pad ducked through the sidechain;
  - the pad ducked by the tempo pump with no audio key, with and without the kick;
  - the pump swung on eighths.
- **Kick and bass:**
  - the whole bass ducked;
  - only its lows under 110 Hz, with minimum and linear phase.
- **The guitar under a voice** (Carbonthief, CC0; the owner's progression):
  - the guitar ducked by a synthetic voice-over, whole, only above 400 Hz, and with the RMS detector (c6, new in A-2);
  - the ducked guitar alone.
- Dry twins for every mix. Each set is scaled by one factor, so levels are real, and the latency is taken off.

## Misses against the targets, honestly

- **The linear-phase latency was 36.8 ms** (1 765 samples at 48 kHz) at every crossover (Phase B (3): now Auto, an octave under the crossover, or a fixed floor — below), plus the lookahead, and a crossover under 60 Hz is minimum phase only. This is the price of the A-2 kernel; the first phase's was 12.7 ms, but it could not reach the depth set.
  - The latency cannot be much lower for the same flatness. To be within 3.6 % at `fc/2` and at −6 dB at `fc`, a kernel needs a transition only `fc/2` wide, which takes about `2/fc` of time either side.
  - A Kaiser-windowed sinc needs more than 1 800 taps at 60 Hz for these bounds, with negative side lobes (a rise of up to 0.1 dB). The searches are in the section *Phase A-2*.
  - Keeping a 50 Hz floor would have cost 44.2 ms. The owner may prefer that, or 80 Hz (27.6 ms): one constant (`band::LIN_MIN_HZ`).
- **A landing on a bar line inside a long cycle** (an 8/1 shape, a dotted division whose cycles do not meet the bar) is not faded, so that a render from that bar equals the whole render. Bar lines are 4/4 from the song's start, as Lapsus counts them.
- **The RMS detector's hold and release count from the RMS crossing**, which comes later than the peak's by the window's rise: for a step to 0.5 against −10 dBFS, 245 samples at 10 ms.
- **A note-off does nothing.** A note plays its cycle, and with `velocity` its depth follows the note's velocity.
- **The shape's cycle counts from the song's start in quarter notes.** A one-bar pattern in 3/4 does not follow the bars, the same open decision as Lapsus's (ADR 0132's Consequences).
- **The classic pump** (`CLASSIC_PUMP`: full depth on the beat, `1 − (1 − u)^2.4` back to nothing by 65 % of the cycle) was tuned by numbers, not by ear: the first version let go by a quarter of the beat and took only 0.6 dB on average. The owner's ear should settle it.

## Verified only in emulation / needs the owner

- Every signal is synthetic except the guitar's DI: the kick (Pulsus), the pad, the bass and the voice.
- The **sidechain from another strip**, the **song position** and the **notes of another track** are all driven by the test driver here, which tells the transport at every block's start and places notes at their offsets. The engine wiring (Phase B) must be tested end to end.
- **Needs the owner's ears:**
  - the release shapes' feel, and whether the S-curve should be the default for voice-over;
  - the classic pump's curve;
  - minimum against linear phase (b3/b4);
  - the voice-over files (c3/c4).
- A `docs/beta-checklist.md` entry belongs to Phase B: "duck a bass by a real kick on another strip, then by the tempo pump; duck a guitar by a real voice-over".

## Phase A-2 — the first phase's misses closed

The integrator asked for four things, DSP only. Every null and bit-exact test of the first phase still passes.

| | Before (phase A) | After (A-2) |
|---|---|---|
| Linear-phase lows at 30 Hz (depth −24 dB, crossover 120 Hz) | −19.84 dB | **−23.92 dB** (asked: within 0.5 dB) |
| Linear-phase highs an octave below (60 Hz) | −1.37 dB of leak | **−0.22 dB** (asked: under 0.3) |
| Linear-phase rise anywhere | none | **none**: `0 ≤ L ≤ 1` by construction (asserted < +0.005 dB) |
| Linear-phase latency, 48 kHz | 610 samples, 12.7 ms (50 Hz floor) | **1 765 samples, 36.8 ms** (60 Hz floor), fixed and reported |
| Linear-phase CPU, lows, sidechain | 0.32 % of a core (quiet machine) | **0.78 %** measured beside six other builds at 100 % load. The unchanged plain sidechain read 0.33 % there against 0.17 % quiet, so the quiet figure is about **0.40 %**. Ten integer boxcars per channel, O(1) at every rate. |
| A seek or start inside a duck | the gain there at once | a **10 ms raised-cosine fade** from the landing; bar lines and cycle starts exempt, so render from bar 5 = the whole render |
| Detector | sample peak | **peak or RMS** (1–300 ms, the family's follower) |
| MIDI velocity | ignored | **a switch**: the depth scaled in dB by velocity |

**(1) The kernel: what was tried, and why this one.** Everything was searched numerically before any code (scripts not committed):

| Kernel | Latency at the floor | `fc/4` | `fc/2` | Rises? |
|---|---|---|---|---|
| Four boxcars (phase A) | 12.7 ms (50 Hz) | 0.959 | 0.846 | no |
| Four boxcars and a 3-tap compensator | 24.5 ms (50 Hz) | 0.995 | 0.938 | no |
| Four boxcars and a sparse FIR fitted by least squares | 48–60 ms (50 Hz) | 1.000 | 1.000 | **yes**: side lobes to −0.04…−0.1 |
| Kaiser-windowed sinc, 1 801 taps | 18.8 ms (50 Hz) | 0.993 (β 4); 1.011 (β 3) | 0.934 (β 4); 0.980 (β 3) | **yes**: side lobes to −0.004 (β 4), +0.1 dB overshoot (β 3) |
| **Smootherstep of two boxcars (chosen)** | **36.8 ms (60 Hz)**, 44.2 ms at 50 Hz | **0.9994** | **0.973** | **no** |

The chosen kernel is the only one found that meets both bounds and **cannot rise**. It also stays O(1) per sample in integers at any rate; a windowed sinc would cost O(taps), a quarter of a core at 192 kHz.

Measured through the DSP: lows −23.92 dB at 30 Hz; highs −0.22 dB at 60 Hz; the drawn response (`response_db`) within 0.01 dB of the measured everywhere. Not ducking, it passes the input to the bit (54 000 samples between the hits).

**(2) The seek fade.** A start or a jump of the transport (not a tempo change) that lands **inside a cycle** fades the shape in over `START_FADE_MS` = 10 ms from the landing sample. A landing **on a cycle's start or on a bar line** is not faded.
- That is what keeps a render from bar N equal to the whole render: the part render starts on a bar line, and the whole render has no start there.
- The fade is a function of the landing alone, so skips and refills agree.
- Measured: a seek 1 000 samples into a full duck starts at > 0.99 of the input, is between 0.2 and 0.8 half-way through the fade, and at the depth exactly 480 samples later. The next cycle ducks at once, as drawn.
- A start at song sample 1 234 is faded. Bar 5 inside an 8/1 cycle is not.

**(3) RMS.** `Detector::Rms` reads the key through `dynfx_follow::EnvelopeFollower` (RMS, attack = release = the window), the family's follower, per channel.
- A 1 kHz key at 0.5 (peak −6, RMS −9 dBFS): the peak fires at −7.5 dB, the RMS does not, and the RMS fires at −10 dB.
- A step to 0.5 fires the RMS at −10 dBFS **245 samples** after it, where the one-pole's own crossing is 245.2.
- It rests exactly (0.97 s after a 50 ms window), and its twins agree.

**(4) Velocity.** With `velocity` on, notes at 127, 64 and 1 duck by **−24.00, −12.09 and −0.19 dB** for a depth of 24, where `24·v/127` asks for −24, −12.09 and −0.19. With it off, all three duck by −24.00.

**Tests added** (`cedo_tests_a2.rs`):
- `the_linear_phase_split_reaches_the_depth_and_leaks_little`
- `a_seek_into_a_duck_fades_in_and_a_bar_line_does_not`
- `the_rms_detector_hears_the_level_and_the_peak_hears_the_peaks`
- `velocity_scales_the_depth_when_asked`
- `the_new_options_keep_the_null`: RMS at depth 0 and at mix 0, velocity at depth 0, linear-phase highs at depth 0, with seeks and notes.

The block-size test now switches to the RMS detector mid-way, and its MIDI run has velocity on with notes at 127, 100 and 40. The rest test has an RMS case.

**Seen failing:**
- The kernel's bounds fail on the phase-A kernel by its own measured numbers (−19.8 against −23.5).
- The seek test fails with the fade forced off (checked by mutation).
- A first version of the fade checked the boundary at the audio's position, which is the lookahead before the landing. It failed render-from-bar-5 with lookahead, and the check moved to the landing sample.

**Sounds:** `b4-bass-ducked-lows-only-linear-phase.wav` is re-rendered with the sharper kernel, and `c6-guitar-ducked-by-the-voice-rms-detector.wav` is new. Every other file is byte-identical to the first render and was left in place. The README and the render log are updated.

## Phase B — the plan

1. **Core** (the integrator's contract):
   - An `EffectKind`, and `PARAMS` mapping every field. The trigger, the release shape, the key filter, the band, the crossover and Listen are choices. Depth shows ∞ at its top, and the division uses the family's list.
   - The keep-the-`out` rule; the shape as content in `EffectSpec` (≤ 64 points, as Lapsus's banks); `Command::SetCedoShape` with its exact inverse and its undo-fuzz arm.
   - A schema bump and fixture.
   - The starting preset (ADR 0071): **Tempo, the classic pump on quarters, 9 dB, lows-only off**. It sounds at once with no routing, and the defaults stay the null.
2. **The engine unit** (`cantus-engine/src/fx/cedo.rs`): an `FxUnit` over `Cedo` with `shape_at` passed in.
   - **The sidechain from another strip** through the existing key routing (Comprimo's and Aequalis's keyed path). The unit must **not be skipped while its key sounds**, even with its own input silent: `must_rest` / the rest check must see the key, as Comprimo's does.
   - **Tempo from `SongPosition`** (`Transport` each pass), as Lapsus.
   - **MIDI from another track**: the note-on of a chosen track routed to the unit at its sample offset. This is a new routing ("a MIDI key", Lapsus's notes path generalised from the track's own notes to another track's), so it needs a model field naming the source track.
   - `latency` from `latency_for`, so the engine's compensation follows the lookahead and the crossover, as for Sibilus's split modes.
   - The telemetry: the reduction now and its history (columns, as Comprimo's meter), the amount, the key's peak and threshold flag, the shape's phase, the tempo.
   - Tests: `tests/fx_cedo.rs` (the null), the rows in `fx_contract.rs`, `rt_memory.rs`, render from bar 5 through the engine, and the bench kernel.
3. **A panel with its own personality**, unlike every existing unit (Comprimo's meters, Lapsus's teal VFD, Cribrum's blue, Fragor's amber):
   - Fine strokes and a **dark central display in both themes**, with a "making room" air. The display shows:
     - **the drawable shape**: a cycle of the division, breakpoints dragged on a grid (Shift free), segments bent by dragging, double-click to add or remove, with the automation editor's segment shapes;
     - the **live gain reduction** drawn over it as a filled trace that scrolls with the song (the playhead from `phase`);
     - in Sidechain mode, the key's level against the threshold line (dragged) and the envelope's attack, hold and release drawn as handles;
     - in Lows or Highs, a small band strip under the display with the crossover dragged and `response_db` drawn at the current amount.
   - Controls grouped: Trigger (Sidechain, Tempo, MIDI, with the division, swing and edge, or the key filter, threshold and lookahead), Envelope (attack, hold, release and its shape), Band (Whole, Lows, Highs, the crossover and its phase), Output (depth, link, mix, output, Listen).
   - **Nothing abbreviated in EN or ES.** Clear label and value units at 75–200 %, legible in every theme, with the frame budget measured with audio playing.
   - **The design sheet first**: Slate and Paper, at rest and playing, beside the references' public screenshots (kept under `refs\`, never committed). Then the owner's approval, then the full editor.
4. **Presets** (ten or more, named and made with care):
   - Classic pump (quarters), Half-time pump, Eighths swung, Sixteenths gate (square), Triplet breathe;
   - Kick sidechain (60 Hz band-pass key, 2 ms lookahead), Kick on the bass lows (110 Hz, minimum phase);
   - Voice-over (S-curve, 300 ms hold, 10 ms lookahead), Voice-over mids (above 400 Hz);
   - Podcast music bed, Snare room (highs), Gentle radio.
5. **CLAP/VST3 export**, bit-identical to the engine:
   - the golden ids appended;
   - the plug-in's sidechain input as the key, and its MIDI input as the trigger;
   - the shape in its state;
   - the latency reported and followed.
6. **Docs:**
   - An ADR on the design decisions above:
     - the amount-linear gain law;
     - the hold from the last sample over the threshold;
     - the S attack;
     - the centred edge kernel;
     - the song-sample grid with swing;
     - the two crossovers and their null paths;
     - the fixed linear-phase latency;
     - the sharing with Comprimo.
   - `docs/plugins/cedo.md`, the manual in EN and ES, the AI tools' description, and the site's card.
7. **End to end** through `CantusApp::assemble`:
   - insert Cedo from the picker on a playing pad track;
   - route a kick track's audio to its key, set the depth, and hear the pad dip on each kick in the master;
   - switch to Tempo and hear the pump with the kick muted;
   - with the linear phase, see the track's latency compensated.
8. **Later** (BACKLOG): a "shape from attack, hold and release" button that draws the envelope as a shape; a note-off that ends a note's shape early (a held-note mode).

## Left (this phase)

- In the DSP: the misses above, which are the owner's to judge. The linear-phase floor and its latency (60 Hz, 36.8 ms) are a choice for him.
- The owner's listening (`compartir\cedo\sound\README.md`).

## Phase B (3), to the finish (2026-11-17): sheet v1 approved as shown

Branch `fx/wave-b` (worktree `C:\projects\daw-fx\wave-b`), after Vinculum's and Lenis's step 3.
The owner approved design sheet v1 as shown, and decided the linear phase's floor mid-step
(below).

### Done

- **The pointer** (`views/fx/cedo_edit.rs`, new; `cedo_display.rs` returns where it drew what
  acts, `cedo_screen.rs` its layout, `cedo_engine.rs` the gauge's place): the shape's points
  dragged (snapped to sixteenths, twelfths for a triplet; Shift free), bent by a drag between
  them, added and taken away by a double-click, a segment's shape turned by a right-click
  (Curve → S → Hold); the envelope's three handles in time and the threshold's line in
  Sidechain; the band strip's crossover and its split's word; the trigger's flag on the bed;
  the gauge and its bellows for the depth; the trigger switch on its card kept for the tests.
  Each gesture one undo step through `SetCedoShape` or the parameter command
  (`FxHost::set_cedo_shape`: Cantus, a board's pedal, the plug-in's window). The pilot lamp
  only shows.
- **`learn_everywhere`'s four** (`rshape`, `kfilt`, `det`, `vel`): drawn with their offer on
  their trigger's cards; the main test excuses them only under the other triggers, and
  `every_cedo_trigger_offers_its_stepped_parameters` draws each trigger and checks every
  stepped key it shows (the floor among them).
- **Seventeen factory presets** (`presets/effects/ducker.toml`) — classic, half-time, eighths
  and swing pumps, triplet breathe, trance gate, reverse swell, lows-only pumps at minimum and
  linear phase, MIDI pump, kick ducks the bass, kick on the bass lows, hard sidechain,
  voice-over, podcast bed, gentle radio, snare room — every trigger and band; five draw
  shapes of their own, which ride the flat map (`cedo::to_flat`, `shape_of_flat`, `flat_is`)
  through presets, A/B, the drift mark, the audition and the agents. A Sidechain or MIDI
  preset is auditioned keyed by the material's drums. Fitted (`preset_tool fit`): the pumps +1.1 … +7.1 dB (Classic pump
  +1.4, in `starting.rs` and the demo too; Reverse swell +7.1, Trance gate +2.3, MIDI pump
  +3.2); the seven Sidechain presets stay at 0 dB by design (`preset_measure::by_design`:
  they duck under the key they are given, and auditioned keyed by the material's own drums
  they duck all of it — the fit had asked up to +13.5 dB of make-up).
- **The linear phase's floor** (the owner, 2026-11-17): `lfloor` (Auto default; 40, 60, 80,
  120 Hz) and `lfhz` (Auto's floor as committed). Auto puts the floor an octave under the
  crossover, committed when the crossover is let go (`DawHost::end_gesture`, into the drag's
  own step; `PluginHost::end_gesture`; a discrete set, a preset, an agent) — never during a
  drag. `band::LIN_MIN_HZ` became `band::Floor`, `FLOORS`, `auto_floor`. Measured at 48 kHz:
  40 Hz 2 655 samples (55.3 ms), 60 Hz 1 765 (36.8), 80 Hz 1 325 (27.6), 120 Hz 880 (18.3).
  Accuracy at every floor (24 dB, crossover an octave over the floor): the lows −23.92 dB an
  octave under the floor, −0.000 dB four octaves over the crossover; the highs leak −0.223 …
  −0.230 dB an octave under the crossover and reach −23.99 … −24.00 dB two over. The Band
  card shows the floor with its latency ("Auto · 60 Hz · 36.8 ms"). Default output unchanged:
  the DSP suite's goldens hold, no `DSP_VERSION` bump, no sound re-rendered.
- **Export** (`Cantus Cedo`, CLAP and VST3): the Direct path; the sidechain input as the key;
  the host's transport for the pump; the host's notes (CLAP's note port in both dialects,
  VST3's event bus); the shape as content in the state (`ContentPost<Shape>` — wave A's
  generalisation of Lapsus's post copied into `lapsus_bank.rs`, its Shape arm instead of
  Fractus's; `state::save_spec_with`, `load_cedo`); the latency reported and re-reported once
  per committed floor; ids appended and blessed.
- **The agents**: `set_effect` takes `shape` (points `x`, `y`, `bend`, `seg`; `null` the
  classic pump) and `notes_track_id` (the track whose notes an effect hears); a crossover set
  commits its Auto floor; aliases `pump`, `sidechain_pump`; the effect words. Decided yes, as
  Fractus's `lanes` set its content. The MCP golden regenerated on purpose.
- **Tabula**: Cedo is a pedal (face: trigger, depth, cycle, threshold); `SetCedoShape` names
  its pedal (`command_board::set_pedal_cedo`), the undo fuzz's board arm draws on one (one new
  pedal in eight a Cedo after the Lapsus draw).
- **The site's card** (EFFECTS 37), ADR 0144 phase B, `docs/plugins/cedo.md`, the manual EN/ES,
  the beta checklist's Cedo entries.

### Measured (the e2e through `CantusApp::assemble`, `tests/cedo_e2e.rs`)

- The classic pump as inserted: 5.1 dB down 0–0.1 s into the beat, 0.01 dB late in it.
- A square shape at ∞: silence from each beat's own sample (24 000·b) for a quarter beat,
  the pad's samples untouched to the bit around it, beats 1–5.
- Live in blocks of 256/333/512/17/1024 = the offline render, to the bit; a new shape heard
  and its undo the render before, to the bit; a render from bar 3 = the whole render's bar 3.
- Lows under 150 Hz at full depth (minimum phase): the bass (55 Hz) 34.5 dB down, the pad's
  A4 0.13 dB.
- The kick strip as the bass's key: 14.9 dB under the kick, 0.000 dB before it plays.
- A muted kick track's notes (MIDI): 12.3 / 12.4 / 12.3 dB at beats 2, 3, 5, 0.000 dB late in
  each beat.
- The rest check: a strip silent until beat 4 (its input under 1e-12 there), keyed by the
  kick and listening to the key, puts the key out from beat 2.
- Two twin pads, Cedo (Lows, linear, depth 0) on one: the song is the dry song 1 765 samples
  late (Auto, 60 Hz) and 2 655 late after the 40 Hz floor is chosen, to the bit.
- The defaults (the key and the notes still routed) = Cedo bypassed, to the bit. (Against the
  song with Cedo *taken out* the master differs by 3e-8 from frame 48 005: the key's routing
  orders the strips and the master's sum rounds differently; bypassed keeps the order.)
- The pointer (`cedo_drag`, 6): the release point to 0.5 / 0.25 on the grid; the first
  segment bent 0.226 → 0.015 at 0.3 of the cycle; the envelope's handles +10.3 ms for 30
  points; the threshold +10 dB; the crossover 120 → 250 Hz where the pointer let go; the
  gauge 9 → 19 dB for 40 points; the crossover dragged to 309 Hz held at 1 765 samples of
  latency until the release, 685 after it, one undo step back to 120 Hz / 60 Hz.

### Shared files touched (marked `fxb:cedo`)

Core: `command.rs` (`SetCedoShape`'s `pedal`), `command_board.rs`, `command_board_tests.rs`,
`fx/tabula.rs`, tests `undo_invariants.rs`. DSP: `cedo_band.rs`/`cedo.rs` (the floor; Cedo's
own). Engine: tests `fx_cedo.rs`. UI: `views/fx/host.rs` (`set_cedo_shape`, `load`,
`save_preset`, `touches`, the floor's commit in `set`/`end_gesture`, `cedo_crossovers`),
`views/fx/mod.rs` (`apply_fx_params`, `preset_map`, `toggle_ab`), `views/fx/frame.rs`
(`modified`), `views/fx/tabula_host.rs`, `views/fx/tabula_faces.rs`, `preset_audition.rs`,
`preset_data.rs`, locales `areas/ai.*`, tests `learn_everywhere.rs`, `preset_audit.rs`, golden
`mcp-tools-list.json`. AI: `tools_mixer.rs`, `tools_args.rs`, `tools_words.rs`, tests
`tools_music.rs`. Plug-ins: `Cargo.toml`, `build.rs`, `src/desc.rs`, `editor.rs`,
`instance.rs`, `lapsus_bank.rs` (wave A's generic post: expect a one-hunk conflict at the
merge, keep both `Content` impls), `state.rs`, tests `common/mod.rs`, `clap.rs`, `vst3.rs`,
`notes.rs`, `transport.rs`, `golden/ids.txt`. xtask: `main.rs`. Web: `units_data.rs`. Docs:
the manual EN/ES, `beta-checklist.md`.

### Lenis's VST3 latency restart on Linux

Checked: `vst3::lenis_s_latency_follows_its_quality_through_vst3` holds its restart counts
inside `#[cfg(windows)]` (the tick is wired only there) and asserts the new latency read on
every system; `vst3/timer.rs` marks its Windows-only paths `allow(dead_code)` off Windows.
Gated correctly; Cedo's VST3 latency test follows the same pattern.

### Verified only in emulation

- The exported plug-in only in our own CLAP/VST3 harness, not in a third-party DAW; the VST3
  latency restart only on Windows.
- The presets have not been heard by the owner; their levels are the fit's loudness match.
- Nothing new was drawn in step 3 but the Band card's floor (whole words, measured in both
  languages by the cards' widths); no new shots.

### Suites (Windows, 2026-11-17, one crate at a time, clippy `-D warnings --all-targets` on each)

cantus-core 811, cantus-dsp 1 131 (the floors' four among them; the goldens unchanged),
cantus-engine 1 311, cantus-project 194, cantus-plugins 92, cantus-ai 51 — clippy clean on all
seven crates (cantus-ui's run again after the catalogue's fix below, clean).

cantus-ui, the whole suite once (`--no-fail-fast`): **2 727 passed, 3 failed** (54 ignored):

- `learn_everywhere::every_stepped_parameter_is_drawn_with_its_offer` — **no Cedo key any
  more**; only Fractus's 16 `Glitch.*` (fixed in fx/wave-a, arriving with the merge).
- `agent_tools::the_catalogue_is_complete_and_generated_from_the_tables` — a Cedo preset's
  flat shape keys read as parameters: fixed (the unit catalogue lists a preset's shape apart
  from its values, as `set_effect`'s `shape` takes it, as Fractus's lanes) and green on its
  own.
- `preset_render::every_factory_preset_renders_clean_level_and_within_its_budget` —
  `preset.arca.tight_rhythm: cpu +0.30`, `preset.arca.dark_lead: cpu +0.28` under the
  suite's load; run again on its own, green. Cedo's seventeen within level (the seven
  Sidechain ones by design) and budget (CPU 0.021 … 0.059).

The MCP tools golden regenerated on purpose (`CANTUS_TOOLS_DUMP=1`: `set_effect`'s `shape`
and `notes_track_id`, Cedo's words).

Not run: the Linux check (WSL), the plug-ins without default features, the release
bundle, the site's pictures and sound excerpts, CPU on a quiet machine, a third-party DAW.
