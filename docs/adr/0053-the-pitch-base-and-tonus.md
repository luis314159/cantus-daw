# ADR 0053 — The pitch base (a tracker and a formant-preserving shifter) and Tonus, the real-time auto-tuner

**Status:** Accepted · **Date:** 2026-10-14 · **Round:** 28 (voice) · **Extends:** ADR 0005
(real-time rules), ADR 0017/0018 (the plug-in family, latency and telemetry), ADR 0020 (our
plug-ins outside Cantus), ADR 0022 (MIDI into effects), ADR 0031 (rest, offline = real time)
· **Serves:** BACKLOG §3d (Tonus) and §3c (the pitch editor, round 28's *pitch* builder)

> Measurements: `docs/round28/voice.md`. The unit's page: `docs/plugins/tonus.md`.

## Context

BACKLOG §3d asks for a real-time auto-tuner (key and scale, any set of notes, MIDI
targets; retune speed from hard to natural; humanise, flex, vibrato; formants kept, a
throat; under 3 ms of latency at 48 kHz, reported; a tracker that does not wobble on
breaths and sibilants), and §3c a Melodyne-class editor for recordings. Both need the same
two pieces — **a monophonic pitch tracker** and **a formant-preserving pitch shifter** —
one streaming at fixed latency, the other offline on a whole take. Nothing existed: no
pitch detection, no PSOLA. Our own implementation, from papers (YIN, de Cheveigné and
Kawahara 2002; pYIN, Mauch and Dixon 2014 — the paper, not its GPL code; TD-PSOLA,
Moulines and Charpentier 1990), in the MPL crate `cantus-dsp`.

## Decision

### 1. The tracker (`cantus_dsp::pitch`)

- **Coarse search** on the mono sum low-passed (64-tap windowed sinc at 48 kHz, cut near
  3.6 kHz) and decimated to about 12 kHz (`Design`: `D = round(sr / 12 000)`): YIN's
  cumulative mean normalised difference over a **20 ms window**, lags from 1 600 Hz down
  to **55 Hz**, the first dip under 0.15 walked to its bottom, a parabola through it.
- **Fine stage at the full rate**: the plain difference function at the lags within
  `D + 1` of the coarse estimate, newest window, and a parabola — the period to a hundredth
  of a sample. Measured: a fraction of a cent on steady vowels from 98 to 784 Hz at 44.1,
  48 and 96 kHz.
- **Voicing** from aperiodicity (< 0.25 to start, < 0.4 to stay), level (−60 dBFS RMS,
  −66 to stay) and the **sibilance share** — the de-esser's cue (`sibilus_detect`: a
  sibilant's energy sits above the voice's band), measured as the share of the window's
  energy above the decimator's band (< 0.5, < 0.65 to stay); two frames to flip either way.
  An octave jump from the last voiced frame keeps the old lag if its dip is nearly as deep
  (a wrong octave never locks in).
- **The lowest f0 is 55 Hz (A1)**: below a bass's lowest sung note (C2, 65 Hz) with a
  margin, and one period (18.2 ms) still fits in the 20 ms window. Lower would lengthen the
  window — more lag on every voice for notes nobody sings.
- **Streaming** (`PitchTracker`): a frame every `32·D` samples (128 at 48 kHz, 2.7 ms), the
  window causal (the newest 20 ms against the lag before); no allocation after `new`;
  exact rest (zero rings tracked by `ZeroRun`, the decision back at "unvoiced, no
  history"); the frame grid counted from the stream's start (`skip` advances it).
- **Offline** (`analyse`): every lag **centred** on the frame (the causal form measures each
  lag half a lag early), a frame every 256 samples at 48 kHz; per frame the pYIN candidates
  (each CMNDF dip takes the Beta(2, 18) weight of the thresholds under which it is the
  first dip; the global minimum a hundredth of the rest), the gate and the sibilance cue
  scaling them; a **Viterbi** pass over 20-cent pitch states × voiced/unvoiced (moves up to
  13 states per frame, a triangle; a 1 % chance of switching voicing; pYIN's 0.5 trust);
  the chosen state's nearest candidate refined at the full rate. Then the **epochs**: per
  voiced stretch, anchored on the largest sample of its first period, one period at a time,
  each aligned to the period before by cross-correlation (coarse on the low band, fine at
  the full rate, a parabola) — so every mark sits on the same phase of the waveform.

### 2. The shifter (`cantus_dsp::psola`)

- **TD-PSOLA with asymmetric windows.** Synthesis marks are spaced by the **target
  period**; each grain is the input around an analysis mark, read at rate `f` (1 keeps the
  formants; `2^(st/12)` moves them: the throat). A grain falls over one input period
  (`P_a / f`) and rises over `min(P_s, P_a / f)`, from the previous synthesis mark. Where the
  windows sum above one (a shift up) the output is divided by their sum; where they dip (a
  shift down) it is not — the dip is what makes the longer period (dividing it out brought
  the old pitch back, measured).
- **Grains on a lattice, phase-aligned.** A new grain's analysis mark is the point of the
  lattice one input period apart, anchored on the previous grain's mark, nearest its
  synthesis mark and no later than the newest input allows. In the streaming form it is
  then aligned to the period before the previous mark by a **causal** cross-correlation
  (both windows to the left of the marks, so nothing unarrived is read; ±P/8, coarse at
  ~12 kHz then fine, a parabola). Without it the tracker's lag during a vibrato showed in
  the output as an 18-cent warble; with it the output's pitch is the synthesis spacing:
  measured 2.3 cents mean, 3.8 worst on a ±40-cent vibrato at speed 0.
- **Fixed latency: 128 samples at 48 kHz** (`LATENCY_48K`, scaled with the rate: 2.67 ms
  everywhere), constant. Output sample `t` is emitted when input `t + L` has arrived; a
  grain created at the previous synthesis mark may read up to the newest sample, so the
  lattice mark nearest the synthesis mark is usable for periods up to about `2L`; below,
  the grain comes from up to a period earlier — the period before, which for a voice is the
  same shape. The tracker may look back 38 ms; the *output* latency stays 128 samples.
- **The dry path is exact.** Not engaged, the output is the input delayed by `L`, bit for
  bit; engaging and releasing cross-fade over 5 ms. Unvoiced input is passed dry unless the
  throat is moved (then grains 4 ms apart, read at `f`).
- **Offline** (`render`): the same engine over a whole take with a **ratio curve** and a
  **formant curve** (by source time) and a **time map** (output → source, piecewise
  linear), grains from the analysis's exact epochs; where the ratio is 1, the formant 0 and
  the map plain (slope 1, a whole-sample offset) the output **is** the source, bit for bit,
  with 5 ms cross-fades around everything else.

### 3. Tonus (`cantus_dsp::tonus`, `EffectKind::AutoTune`)

- **Targets**: the allowed pitch class nearest the sung pitch (scale + per-note states
  *as the scale / on / removed / bypass*), or the pitch classes held on MIDI, or the newest
  MIDI note exactly; a new note must be 0.12 semitones nearer before the target changes.
  The project has no key: Tonus has its own `key` (no model change).
- **Correction**: `c → (T − p)·flex` by a one-pole at the retune speed (95 % in `speed`;
  humanise lengthens it up to 400 ms on notes held past 120 ms); output pitch
  `p + amount·c + vib·through·(p − p_centre) + created vibrato + transpose`, glided 2 ms;
  the shifter's ratio is the output pitch over the detected one, so at speed 0 the output
  is the target whatever the voice does.
- **MIDI at the sample** (ADR 0022): the slot's notes are queued with their offsets and
  applied at their sample, re-deciding the target there. In the **exported plug-ins**, every
  kind that takes notes (Tonus, Porta) now has a **CLAP note port** (CLAP and MIDI dialects)
  and a **VST3 event bus**; `instance::Processor::process_with_notes` hands each note to
  the unit at its own sample inside the grid's pieces.
- **The nulls** (Tonus corrects at its defaults — a chromatic tuner at 20 ms, what a
  singer expects): **amount 0**, every note on **bypass**, a **MIDI source with nothing
  held**, and the slot's bypass are the input delayed by 128 samples, bit for bit — through
  the whole engine, with delay compensation holding the rest of the mix back with it.
- **Rest**: exact once the tracker's and shifter's rings hold zeros, no note is held and no
  correction is under way (0.35 s of silence at 48 kHz).
- **Telemetry** (64 floats, `cantus_engine::fx::tonus`): the sung pitch, target, shift,
  flags, held/target/bypass classes, level, and 24 columns (5.3 ms each) of sung and output
  pitch, counted from the stream's start, for a frame-rate-independent display.

### 4. The API the pitch editor builds on (round 28's *pitch* builder)

```rust
use cantus_dsp::pitch::{analyse, Analysis, Epoch, PitchFrame, hz_to_midi, midi_to_hz};
use cantus_dsp::psola::{render, Plan, TimeMap};

// Analysis of a whole take (mono; sum a stereo take first). Deterministic.
let a: Analysis = analyse(&mono, sample_rate);
a.hop;                     // samples between frames (256 at 48 kHz); frame i centred on i·hop
a.frames[i]: PitchFrame {  // per hop:
    centre, f0 /*Hz, 0 = silence*/, period /*samples*/, voiced,
    aperiodicity, level_db, sibilance /*0..1*/, voiced_prob }
a.f0_at(t) -> Option<f32>  // interpolated f0 at sample t (None where unvoiced)
a.frame_at(t), a.voiced_runs() -> Vec<(first, end)>
a.epochs: Vec<Epoch { at /*fractional sample*/, period }>   // phase-locked pitch marks

// Resynthesis: any ratio and formant curve over source time, any time map.
let out: Vec<Vec<f32>> = render(
    &[&left, &right],      // the take's channels (the same length as analysed)
    &a,
    &Plan {
        ratio: &|src_t| ratio_at(src_t),        // 1 = unchanged; 2^(cents/1200)
        formant: &|src_t| semitones_at(src_t),  // 0 = formants kept
        time: TimeMap::new(vec![(out_t, src_t), …]).unwrap(), // or TimeMap::identity()
        len: output_len,
    },
);
```

- **Speed** (release build of `cantus-dsp`, one core): analysis **0.85–0.96 s per minute**
  of audio at 48 kHz; the renderer is a few grains per sample (well under a second per
  minute).
- **Guarantees**: both deterministic (the same bits twice; `cantus_dsp::math` only); the
  renderer is the source **bit for bit** wherever the plan does nothing (so an edit never
  touches its neighbours) and cross-fades 5 ms into and out of every change; the time map
  continues at slope 1 outside its breakpoints.
- **Not done here, for the pitch builder**: note segmentation (blobs, drift, vibrato kept
  apart), key detection, the cache, and the project model of the edits.

## Consequences

- The latency is the owner's target (≤ 144 samples at 48 kHz) with room to spare, and it is
  the same at every setting, so delay compensation never moves while a singer works.
- A singer monitoring through Tonus hears 2.7 ms plus the interface's round trip — but
  delay compensation aligns every strip to the project's largest latency, so a
  long-latency plug-in elsewhere delays the monitored voice too (measured: a linear-phase
  EQ on another strip makes it 4 608 samples). That is the engine's rule, not Tonus's;
  BACKLOG §4c's recording mode is the remedy.
- Polyphonic input is out of scope (it tracks the strongest periodicity). Harmony voices
  (§3d "later") would reuse the shifter with a second synthesis lattice.
- The exported plug-ins' note input closes the gap for Porta's MIDI trigger too.

## What the tests hold

- `cantus-dsp` `pitch_tests.rs`: steady vowels (98–784 Hz, 44.1/48/96 kHz) with no gross
  error and under 2 cents worst, streaming and offline; vibrato and glides followed (mean
  ≈ 2 cents); the octave trap; 58 Hz tracked; a phrase with a breath and an "s" — voicing
  above 97 %, no gross error; exact rest; offline deterministic and under 6 s per minute;
  epochs one period apart and phase-locked (consecutive periods correlate above 0.95).
- `psola_tests.rs`: the latency (≤ 144 at 48 kHz, < 3 ms everywhere); released = the input
  delayed, bit for bit; unity reproduces the voice (pitch within 0.5 cent, envelope 0.5 dB,
  level 0.2 dB); the output pitch is the ratio (±3 to ±12 st, within 2 cents); formants kept
  (≤ 3 dB) and moved when asked; a ratio jump makes no click; exact rest; tracker and
  shifter together land on a target; offline: nothing to do is the take bit for bit, a
  shift keeps the formants and the untouched part bit for bit, a 1.5× stretch keeps the
  pitch, deterministic.
- `tonus_tests.rs`: the nulls bit for bit; speed 0 without warble; a natural speed corrects
  with no click and keeps the vibrato; scales and removed notes; `choose`; MIDI targets on
  their sample and the same bits at every block size; the melody source; every block size;
  exact rest; the throat moves formants, not pitch.
- `cantus-engine` `fx_tonus.rs`: the latency; the nulls through the whole engine are the dry
  mix delayed by exactly 128; block-size independence while correcting; offline = real
  time; MIDI targets from another track's clip; monitoring late by 128 only, and by the
  project's largest latency when there is one; `fx_contract.rs` (split invariance, exact
  rest twins) at defaults, heavy and random settings.
- `cantus-plugins` `notes.rs`: the note port / event bus on exactly the kinds that take
  notes; CLAP (both dialects) and VST3 notes give the DSP's bits at every block size;
  `clap.rs`/`vst3.rs` bit-identical to the DSP, `ids.txt` blessed.
- `cantus-ui` `fx_tonus.rs`: the editor paints the detected note, target, shift and cards;
  columns taken once through the wrap; the keyboard (click, right-click, one undo each,
  back to the scale); *More*; presets in range; the preview; the demo project opens and
  plays corrected.
