# Comprimo — the compressor

Effect kind `Compressor`. Parameter model `crates/cantus-core/src/fx/compressor.rs`; DSP
`crates/cantus-dsp/src/comprimo.rs` with `comprimo_loudness.rs` (BS.1770) and
`comprimo_oversample.rs` (half-band oversampling); engine glue `comp_settings` and the
`Dsp::Comp` arms in `crates/cantus-engine/src/effects.rs`; editor
`crates/cantus-ui/src/views/comp_editor.rs`; strings `crates/cantus-ui/locales/fx/
compressor.{en,es}.txt`. Benchmark: FabFilter Pro-C 3 (`research/fabfilter/pro-c.md`).

## What it does

A feed-forward gain computer (soft knee, range) behind a detector you can read: peak or
RMS, feed-forward or a share of feedback, program-dependent release, a second slower
release stage, and an exponential or linear release. Around it:

- **Time**: attack 0.005–250 ms, release 5 ms–5 s (with a snap to note values at the
  project's tempo), hold 0–500 ms, auto release, lookahead 0–20 ms (a true latency,
  reported), oversampling 1×/2×/4× of the gain stage and the detector (31 / 36 samples of
  latency).
- **Output**: wet (make-up) gain ±36 dB, dry gain (off to +12 dB) for parallel compression,
  mix 0–200 % (scales the gain change in dB: 0 % is a wire, 200 % doubles it), input and
  output gain ±24 dB, **measured auto gain**.
- **The key**: this strip or any other (chosen in the editor, no send), its level ±24 dB,
  a one-pole high-pass, and a **six-band EQ on Aequalis's matched design** (all nine
  shapes, cuts 6–96 dB/oct) with its own bypass; stereo link 0–100 %.
- **Listening**: Output, Wet (no dry), Key (after level, high-pass and EQ), Delta (what the
  compression takes away), and **Compare** — the input bypassed at the output's measured
  loudness.
- **Nine styles**, each a named `StyleSpec` (five numbers), and an **engine drawer** that
  edits them ("Custom (from Opto)").

## Decisions, and why

1. **Keys kept, ranges widened, the map made sparse.** The thirteen keys of the first
   release keep their meaning and defaults (a test holds it); knee grew to 72 dB, attack
   down to 5 µs, make-up to ±36, style to nine, listen to five. With 70 parameters (six
   key-EQ bands of seven fields) the compressor stores only what moved, as the EQ does; a
   dense map from an older file loads unchanged. No schema bump.
2. **Mix scales the gain change in dB**, as Pro-C's does, so 200 % is twice the
   compression and 0 % is bit-exact. The dry path is separate (parallel compression), and
   the output gain comes after both.
3. **Feedback is modelled as the detector hearing the key times the gain in force**
   (`d = k·((1−fb) + fb·g)`), which works for an external key and through the key EQ, and
   whose steady state has a closed form. The transfer curve draws the *effective* curve —
   solved by bisection — with the set one dashed behind, and prints the ratio a feedback
   detector really gives ("feedback: really 2.6:1"). Pro-C's Vari-Mu confusion is exactly
   this, unexplained.
4. **Auto gain is measured, not estimated.** Short-term loudness (K-weighted, 3 s) of the
   input and of the compressed path *before* make-up, gated at −50 LUFS, on a 100 ms grid;
   the difference glides in over 0.5 s. Both meters warm up identically, so the difference
   is right from the first 100 ms and no estimate is needed. With nothing compressed the
   two streams are the same samples, so auto gain is exactly 0 dB — the null test holds
   with it on.
5. **Compare is a listen mode**, so it is one parameter, one undo step, and automatable:
   the input delayed by the latency, at `L_out − L_in` (short-term, gated, smoothed).
6. **Meters report what the compressor makes**, not what an audition plays: while you
   listen to the key or the delta, loudness, statistics and the output meter still read
   the output.
7. **Integrated loudness lives in the editor.** The DSP reports the output's last eight
   400 ms blocks with a step counter (so a slow frame loses none); the editor gates them
   (BS.1770: −70 LUFS absolute, −10 LU relative) over what played while it was open, with a
   reset. Resetting a DSP-side integrator would have needed a parameter that is not one.
8. **Oversampling is linear-phase half-bands** (63 taps, Kaiser β 7.86; 19 for the second
   stage), whole-sample latency by construction (half a sample of padding at 4×). The key
   goes through the same up-sampler, so the detector hears true peaks and stays in step
   with the audio it controls; the gain ramps linearly across the high-rate phases. Off, the
   path is untouched and bit-exact.
9. **The response graph runs the real DSP** (at 16 kHz, cached per setting) over a burst
   12 dB over the threshold for 400 ms, and measures attack and release as 63 % points.
   It is what "attack" means in this style.
10. **Cheap where it runs every sample.** Gains are one `exp` (not `powf`); the release
   coefficient, which auto and program release move every sample, is a four-term series
   (exact to parts in a billion for time constants over a millisecond); auto gain and the
   compare glide on a 16-sample grid; the delay lines index without division; the
   half-bands sum two slices with no bounds checks. Measured on the build machine while it
   ran eight other builds (so these are upper bounds): about 220 ns per stereo sample plain,
   250 with two key-EQ bands, 420 at 2× and 600 at 4× — 1.0 %, 1.2 %, 2.0 % and 2.9 % of
   one core at 48 kHz.
11. **History in frames, grid from the transport.** Columns are the engine meter's maxima
   per frame (nothing between frames is lost); each column stores the transport position,
   so bars and beats are drawn behind it. It freezes when the transport is stopped and
   silent, scrolls back with the wheel, and the Whole-take speed folds pairs of columns
   instead of forgetting.

## What the tests hold

DSP (`cantus-dsp`, `comprimo::tests` and `comprimo::loudness::tests`):

- **Null**: below threshold, every style, with auto gain on, at mix 200 %, with 5 ms of
  lookahead: the output is the input delayed by exactly the reported latency, to the bit,
  and auto gain reads 0 dB. Mix 0 % is bit-exact while compressing hard.
- **Block size**: all nine styles, plus a configuration with everything on (lookahead,
  key EQ, custom engine with feedback, dual and linear release, auto gain, dry, mix, in/out
  gain), plus Compare, plus 2× and 4× oversampling: identical samples **and telemetry** at
  blocks 1, 7, 64, 1000 and 4096 against 512.
- **Measured behaviour**: 4:1 settles at −15 dBFS on a 0 dBFS tone 20 dB over (±1 dB); ∞:1
  holds the threshold (±0.1 dB); mix 50 % / 200 % give half / double the reduction in dB
  (±0.3 / ±0.5 dB); the response graph measures 10 ms attack as 10 ± 1 ms and 100 ms
  release as 100 ± 5 ms; 100 ms of hold adds 100 ± 2 ms to the release; a feedback style
  settles on its effective curve within 0.1 dB; a custom engine equal to a style is that
  style to the bit.
- **Loudness**: 997 Hz at 0 dBFS in one channel reads −3.01 LKFS ±0.05 at 44.1, 48 and
  96 kHz; the K-weighting at 48 kHz matches BS.1770's printed coefficients to 1e-6; gating
  drops silence and passages 20 LU down. Auto gain matches output to input within 0.5 LU
  on drums compressed by more than 3 dB on average; Compare is within 0.5 LU of the output.
- **Key EQ**: a 300 Hz, 48 dB/oct low cut on the key stops a 60 Hz tone from compressing
  (GR < 0.5 dB, from > 10 dB); its bypass restores the exact reduction; a +12 dB bell makes
  a −26 dBFS tone trigger, and Key listen hears it 12 ± 0.3 dB up.
- **Oversampling**: 2× and 4× with nothing to compress are a delay of exactly 31 / 36
  samples, flat within ±0.01 dB to 18 kHz, error under −60 dB; the 2× image of a 20 kHz
  tone is at least 70 dB down; a 12 kHz sine whose samples sit at 71 % of its peak reads
  0 dB (±0.1) at 2× and 4× and −3 dB at 1×.
- **Robustness**: at every range's extreme the output is finite, and after silence the
  filters and the RMS window reach exact zero (a steep low cut on the key is stopped as a
  whole band once it hears silence, so it does not ring at the flush level).

Engine (`crates/cantus-engine/tests/fx_compressor.rs`): three settings that cannot
compress render bit-identical to no compressor; the lookahead's reported latency (240 at
5 ms) is exactly the delay of the render; everything on is block-size independent through
the engine; a key EQ decides whether another strip can duck this one; the unit reports its
telemetry.

UI (`crates/cantus-ui/tests/fx_compressor.rs`): every part paints with a demo running, in
English and Spanish, with no raw string key (the language is global, so these tests take
turns); dragging the curve's slope sets the ratio on
its click stops (Shift: free) as one undo step; dragging the threshold line on the history;
the engine drawer's first turn makes the style custom from its own numbers in one undo
step; a key band is made where you double-click and removed without a trace; "Duck this
strip from…" is one undo step and refuses a loop; the desk draws a GR lane on a strip with
a compressor, filled once the compressor reports reduction; every factory preset sets real parameters; the Whole-take history
folds instead of forgetting.

## Against Pro-C 3

| | Pro-C 3 | Comprimo | |
|---|---|---|---|
| Threshold, ratio 1–∞, knee 0–72, range 0–60 | ✓ | ✓, ratio with click stops | have |
| Attack 0.005–250 ms, release, hold, auto release | ✓ | ✓, release 5 ms–5 s | have |
| Lookahead 0–20 ms, latency | ✓ | ✓, **latency always shown** | better |
| Mix 0–200 %, wet/dry gain, in/out gain | ✓ | ✓ | have |
| Oversampling | up to 32× | 2×, 4× (the realistic range per reviews) | have (less) |
| Auto gain | an estimate | **measured, gated short-term LUFS** | better |
| Equal-loudness compare | — | ✓ | better |
| Listen modes | one headphone (delta) + key | Output / Wet / Key / Delta / Compare, named | better |
| Styles | 14 black boxes | 9, each a visible detector, **engine drawer** | better |
| Response graph | — | ✓, with measured attack/release | better |
| Side-chain EQ | 6 bands, no global bypass | 6 bands, Aequalis design, **global bypass**, key listen | better |
| Side-chain from another track | host routing (6 steps/DAW) | chosen in the editor, any strip | better |
| Level/GR history | read-only, one speed | **drag the threshold**, Fast/Slow/Whole take, freeze, scroll back, hover readout, **bars and beats** | better |
| Knee display | read-only | drag threshold, ratio, knee, range; wheel = knee; effective curve for feedback | better |
| Loudness | momentary only | momentary, short-term, integrated | better |
| GR statistics, crest factor | — | 10 s average / peak, crest in→out | better |
| Input meter | hidden in a menu | always shown | better |
| Undo | local to the plug-in | the project's, one step per gesture | better |
| Presets, A/B | ✓ | host presets + A/B as undo steps; 8 factory presets | have |
| GR on the mixer strip | — | a lane beside the strip's meter (live for the open editor today; see below) | have (partial) |
| Tempo-synced release | — (debated on KVR) | a snap to note values at the project tempo, ms kept | better |
| Duck from a strip | — | ✓, one undo step | better |
| Character (saturation) | Tube / Diode / Bright | — | missing |
| Auto threshold | ✓ | — | missing |
| Host-sync and MIDI trigger | ✓ | — | missing |
| Stereo link modes (M/S, M>S, S>M) | ✓ | L/R link only | missing |
| Upward / TTM styles | ✓ | — (downward only) | missing |
| Per-band M/S on the key EQ | ✓ | stereo only | missing |
| Surround | up to 9.1.6 | stereo (Cantus has no multichannel bus) | missing |

**Missing, and why.** Character, auto threshold, host-sync/MIDI triggers, M/S linking and
upward modes are P2 in the research and each is a feature of its own; they were left out
rather than half-built. Surround waits for multichannel buses. Oversampling stops at 4×:
the reviews call 32× impractical, and 4× already puts a fast gain change's sidebands
under an 80 dB filter.

## For the orchestrator

- The strip GR meter reads the open editor's telemetry; for every compressor on the desk
  to read live, the engine needs a per-strip reduction in `Meters` (e.g.
  `strip_gr: [f32; MAX_STRIPS]`, the largest `comp_meter().gr_db` of a strip's units each
  block) and `CantusApp::comp_gr_on_strip` should read it.
- Lookahead and oversampling report their latency through `EffectUnit::latency()`
  (`Comprimo::latency()` = lookahead + oversampling); delay compensation will line them up.
