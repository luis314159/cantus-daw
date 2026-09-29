# Modulus — the FM synth

*Round 17, ADR 0023. Research: [`research/instruments/fm-additive-granular.md`](../../research/instruments/fm-additive-granular.md)
(the FM parts). Kind `InstrumentKind::Fm`. Files: `crates/cantus-core/src/inst/fm.rs` (+ `fm_algorithms.rs`),
`crates/cantus-dsp/src/modulus*.rs`, `crates/cantus-engine/src/inst/fm.rs`, `crates/cantus-ui/src/views/inst/fm*.rs`,
`crates/cantus-ui/locales/inst/fm.{en,es}.txt`; tests `crates/cantus-engine/tests/inst_fm.rs`,
`crates/cantus-ui/tests/inst_fm.rs` and the DSP's own `modulus_tests.rs`.*

## What it is

Eight operators, A…H, any to any. An operator is a phase accumulator reading our own sine table
(or band-limited triangle, saw and square tables, one per octave), scaled by its envelope, its
level, its key scaling and its velocity. The **matrix is free**: every cell `m.<s>.<t>` is the
peak phase deviation, in radians, that operator *s* at full amplitude puts on *t*; the diagonal
is feedback. Every cell is an ordinary number of the spec, so it is automatable, modulatable
by the shared modulation matrix, and undoable — Sytrus's matrix cannot be automated, FM8's
shows no modulation on its cells.

- **Evaluation order is fixed and documented**: H first, A last (the DX7's 6 → 1). An edge from a
  later letter to an earlier one uses this sample; the other way round — a cycle — uses the
  previous one, drawn dashed and marked z⁻¹ in the graph.
- **Feedback and back edges go through the DX7's two-sample average** (Tomisawa's anti-hunting
  filter: a zero at Nyquist), on by default, a switch (`fb_avg`). The test shows why: at
  β = 1.5–2 rad the plain one-sample loop puts −6 dB of its energy in the top 1 % of the band
  (the limit cycle), the averaged loop −45 to −55 dB.
- **The 32 DX7 algorithms are templates** (`DX_ALGORITHMS`, facts written from the published
  chart): picking one writes every cell, feedback and output of the topology, keeps the indices
  already there, says how many cells it changes, and leaves every cell free afterwards. Carriers
  get `−10·log₁₀(n)` dB each, so switching algorithm keeps the loudness and the headroom.
- **Frequencies**: ratio (0.0625…64, four decimals) or fixed (0.1 Hz…20 kHz), detune in cents,
  and FM8's offset in Hz (beating that stays constant across the keyboard).
- **Envelopes**, per operator, in two modes:
  - **DX**: four rates and four levels 0…99, driven by the published DX7 measurements — levels in
    0.0235 dB steps through the DX output-level table, `qrate = R·41/64`, decay linear in dB at
    `0.2819·2^(qrate>>2)·(1 + (qrate&3)/4)` dB/s, the attack's jump to 40 dB above the floor and
    its concave climb, keyboard rate scaling 0…7. The floor is silence.
  - **Time**: up to eight breakpoints in milliseconds with a bend per segment, a sustain point
    and a loop back to an earlier point while the key is held, time scaling by key and by velocity.
  - Both restart from where they are on a stolen voice (no click).
- **Keyboard level scaling**: the DX break point, a depth per side (dB at two octaves; DX depth 99
  = 48 dB) and the four DX curves (−lin, −exp, +exp, +lin). **Velocity sensitivity** per operator
  (at 100 %, velocity 0.1 is 40 dB down).
- **Voice**: poly/mono/legato with the shared allocator, glide, unison 1–8 with detune and spread
  (each copy's phase seeded from the note), seeded per-operator drift, a pitch envelope, two LFOs
  (LFO 1 also as DX-style vibrato and per-operator tremolo), a modulation envelope, bend range.
- **The easy macros** (FM8's Easy page, non-destructive): Brightness (every index ×4 at +100 %),
  Harmonic (modulator ratios stretched `r^(1 + h/4)`), Detune, and Attack/Decay/Sustain/Release
  offsets. They are offsets until **Bake** writes them into the patch in one undo step — and the
  test shows the baked patch renders the same samples as the unbaked one.
- **The morph pad** (FM8's Morph Square): four corners, each a snapshot (text keys
  `morph.c1…c4`) of every operator's ratio, fixed frequency, detune, level, output and feedback,
  every matrix cell and three easy macros; `morph.x`/`morph.y` mix them bilinearly. A corner never
  stored stands for the patch as it is, so the pad does nothing until one is stored.
- **Shared modulation**: the eight macros and 32 slots. Sources: env1, lfo1, lfo2, vel, relvel,
  key, rnd, mpe.bend/press/slide, macro1…8, mw, at, pb, expr, sus. Targets: gain, tune, every matrix
  cell and feedback, every output and pan, each operator's level, detune and ratio, the LFO
  rates, vibrato and tremolo depth, and the easy macros (other keys are ignored as targets).
- **MPE**: per-note tuning, pressure, slide, pan and volume through the contract's note
  expressions (pressure and slide as `mpe.press`/`mpe.slide` sources).

## Aliasing, saved with the patch, and render = playback

`quality` (Eco 1× / Auto / 2× / 4× / 8×) and `guard` (Off / Show / Tame) are **patch values**.
Each voice runs at 1, 2, 4 or 8 times the host rate, chosen **once, at its note-on**; it comes back
through a cascade of polyphase IIR half-bands (12 coefficients, Valenzuela–Constantinides; pass
band to 0.45·fs, stop band from 0.55·fs, under −80 dB). **Auto** picks the smallest factor whose
band can carry the voice's bandwidth: content folds into the audible band only beyond
`(N − ½)·fs`, so the voice needs its bandwidth under `(N − ½)·fs·0.9`. The bandwidth is Carson's
rule run through the graph in evaluation order (a modulated modulator counts with its own
bandwidth), with the carriers' own sidebands counted down to about −60 dB
(`I + 2.5·∛I + 2` of them) and feedback as about sixteen harmonics per radian. **Tame** scales the
voice's indices (by bisection) until that bandwidth fits the factor.

**Feedback and back edges are delayed by one host sample whatever the factor** (N sub-samples at
N×), so the quality changes the aliasing and never the timbre: the test measures the first eight
harmonics of a feedback patch at 1×, 2×, 4× and 8× within 0.5 dB — Sytrus's manual warns that its
oversampling changes the feedback sound. Everything else is on a 16-sample control grid counted
from the stream's start, every random number is seeded from the note, and the render is the same
code as playback, so there is nothing to differ.

## Parameters

Keys are forever. `<x>` is an operator letter a…h.

| Key | Range | Default | Unit |
|---|---|---|---|
| `gain` | −48…12 | 0 | dB |
| `voices`, `mode`, `priority` | 1–64; poly/mono/legato; last/low/high | 16; poly; last | |
| `glide` | 0–5000 | 0 | ms |
| `unison`, `uni.detune`, `uni.spread` | 1–8; 0–100; 0–100 % | 1; 10; 50 % | ; ct; % |
| `transpose`, `tune`, `pb_range` | ±48; ±100; 0–48 | 0; 0; 2 | st; ct; st |
| `quality` | Eco 1× / Auto / 2× / 4× / 8× | Auto | saved with the patch |
| `guard` | Off / Show / Tame | Show | |
| `fb_avg` | off/on | on | the two-sample average |
| `drift` | 0–20 | 0 | ct, seeded per note |
| `penv.l0…l3`, `penv.t1…t3` | ±48; 0–10 000 | 0; 100 | st; ms |
| `lfo1.rate/wave/delay/sync/pitch/amp` | 0.01–50; 6 waves; 0–5000; ; 0–1200; 0–48 | 5; sine; 0; on; 0; 0 | Hz; ; ms; ; ct; dB |
| `lfo2.rate/wave/delay/sync` | as LFO 1 | 2; triangle; 0; on | |
| `menv.a/d/s/r` | 0–10 000; 0–20 000; 0–100 %; 0–20 000 | 10; 500; 0; 300 | ms, % |
| `macro.bright/harm/detune/a/d/s/r` | ±100 % | 0 | offsets until Bake |
| `morph.x`, `morph.y`; `morph.c1…c4` (text) | 0–100 % | 0 | the pad; its corners |
| `op.<x>.on`, `.wave` | off/on; sine/tri/saw/square | on; sine | |
| `op.<x>.freq_mode`, `.ratio`, `.fixed` | ratio/fixed; 0.0625–64; 0.1–20 000 | ratio; 1; 440 | ; ×; Hz |
| `op.<x>.detune`, `.offset` | ±100; ±100 | 0 | ct; Hz |
| `op.<x>.level`, `.vel` | −96–0; 0–100 % | 0; 0 | dB; % |
| `op.<x>.phase`, `.sync`, `.penv`, `.ams` | 0–360; ; ; 0–3 | 0; on; on; 0 | ° |
| `out.<x>`, `pan.<x>` | −60 (off)…+6; ±100 % | A 0, others off; 0 | dB; % |
| `fb.<x>` | 0–4 | 0 | rad |
| `m.<s>.<t>` (s ≠ t) | ±16 | 0 | rad of peak phase deviation |
| `env.<x>.mode` | time/DX | time | |
| `env.<x>.n`, `.sus`, `.loop`, `.l0` | 1–8; 0–8; 0–8; 0–100 % | 3; 2; none; 0 | points |
| `env.<x>.t1…t8`, `.l1…l8`, `.c1…c8` | 0–30 000; 0–100 %; ±100 % | ADSR 5 ms/100 %, 300 ms/70 %, 250 ms/0 | ms; %; bend |
| `env.<x>.key_time`, `.vel_time` | ±100 % | 0 | per octave; by velocity |
| `env.<x>.dr1…dr4`, `.dl1…dl4` | 0–99 | 99; 99 99 99 0 | DX |
| `ks.<x>.break`, `.ldepth`, `.rdepth` | 0–127; 0–48; 0–48 | 60; 0; 0 | note; dB at two octaves |
| `ks.<x>.lcurve`, `.rcurve`, `.rate` | −lin/−exp/+exp/+lin; 0–7 | −lin; 0 | |
| `name` (text) | | | the patch's name (a DX voice's) |

## DX7 SysEx

`views/inst/fm_sysex.rs` reads and writes the documented formats — a single voice (`F0 43 0n 00 01
1B`, 155 bytes) and a 32-voice cartridge (`F0 43 0n 09 20 00`, 4096 packed bytes) — clamps and
counts out-of-range values, reports a wrong checksum but still loads, and round-trips byte for
byte. A voice becomes a patch through the published scalings (output level through the DX table,
0.75 dB per step; ratio coarse × (1 + fine/100), coarse 0 = 0.5; fixed 1/10/100/1000 Hz ×
10^(fine/100); break point + 21 = MIDI note; depths × 48/99 dB; feedback *n* = π·2^(n−7) rad; the
algorithm through the templates with a modulator at level 99 worth 4π rad) and arrives as **one
batch, one undo step**. A cartridge shows its 32 names to pick from. Approximations, named in the
code: the LFO speed and delay in hertz and seconds, the pitch-envelope times, the vibrato depth
per sensitivity, DX detune as one cent per step.

## The editor

*(Screenshots are taken with `cargo run -p cantus-ui --example shoot -- <dir> 1 fm`; they are
not committed.)*

- **Play**: the operator graph and the spectrum, large, and the easy macros with Bake.
- **Edit**: the graph (or the matrix grid), the operator shell — every operator's wave, ratio or
  frequency, detune, level, velocity, output, feedback and envelope thumbnail as numbers you drag
  — the spectrum, the selected operator's envelope / key scaling / oscillator pages, and the voice.
- **The graph**: operators laid out by depth over OUT; drag from one onto another to connect,
  onto itself for feedback, onto OUT to hear it; each edge's index is a number you drag
  (double-click removes it); back edges dashed with z⁻¹; a ring on a number a modulation source
  reaches; each operator's live amplitude as an arc (telemetry).
- **The matrix grid**: FM8's matrix with every value printed, tinted by strength, a ring where a
  source reaches it.
- **The spectrum** is the patch's own sound, rendered by the same DSP on the note the engine last
  played, with the Carson edge, the Nyquist line, the voice's rate and the guard's verdict — amber
  when sidebands fold back.
- **Envelopes are drawn and dragged**: time mode in square-root time (a 2 ms attack and a 4 s
  release both visible), points dragged in time and level, a diamond per segment for its bend,
  right-click for sustain, loop and removal; DX mode with the four points dragged up (level) and
  sideways (rate) and the numbers beside it. **Key scaling** as a curve over the keyboard with the
  break point and each side's depth as handles.
- **DX 1–32** as pictures of their topologies; **Import DX7…** for `.syx` files.
- **One hue** (a teal chosen per theme for contrast), knobs from the shared `param_knob`, every
  value in its unit.

## Presets

Ten, each naming and wiring its macros: Tine Piano (Tine, Bark), Feedback Bass (Growl), Chowning
Bell (Strike), FM Brass (Blare, Vibrato), Marimba (Mallet), Drawbar Organ (Upper), Glass Pad
(Shimmer, plus an LFO on the index), Pluck Lead (Bite), Metal Perc (Ring), Breathy Flute (Breath).

## What the tests hold

**Model** (`cantus-core`, `inst::fm::tests`): the table is laid out in blocks `param` searches
(every key found, the diagonal is `fb`), a fresh patch is operator A alone, the 32 algorithms are
well formed (every operator sounds or modulates, none does both, each matches itself), a template
writes the whole topology including the loops of algorithms 4 and 6.

**DSP** (`cantus-dsp`, `modulus_tests.rs`): the half-band passes 0…0.45·fs within 1 % and stops
0.55·fs and up below −80 dB; the DX level tables are the published mapping; DX decay speeds match
the published formula within 1 % at rates 20…80; the attack jumps 40 dB then climbs concave; a
breakpoint envelope holds, loops and releases; the default patch is a sine at the key; every
block size (1, 7, 64, 512, 1000) renders the same samples with feedback, 4× and an LFO; a voice
ends in exact zeros; Carson's estimate covers the measured 99 % energy for five ratio/index pairs;
feedback 0…4 rad stays bounded, and where the plain loop hunts (β 1.5 and 2) the average keeps
the top 1 % of the band 38–48 dB emptier.

**Engine** (`cantus-engine/tests/inst_fm.rs`, run with `--nocapture` for the numbers):

| Test | Result |
|---|---|
| a rich patch (algorithm 5, feedback, a back edge, DX envelope, LFO, unison, glide, drift, two routes) at block sizes 1, 7, 64, 512, 1000 and twice | identical, tolerance 0 |
| every default written out | the same samples as a fresh patch |
| eight-note chord at velocity 127 | −6.4 dBFS (default), −3.9 dBFS (the rich patch); exact zeros after |
| pitch, A0…C8 every third key | at most **0.0002 cents** off |
| alias at C7, 1:1, exactly periodic (dB, whole band / below 20 kHz) | index 8: Eco −29.5 / −47.7, **Auto −80.0 / −98.1**, Eco+Tame −95.0 / −97.3; index 16: Auto −64.0 / −97.1 |
| feedback 2 rad at 1×, 2×, 4×, 8× | the first eight harmonics within 0.5 dB |
| the morph pad | halfway mixes, unstored corners are the patch, no corner = nothing |
| spectral centroid below 16 kHz at 44.1, 48 and 96 kHz | within 1 % |
| MPE tuning +12 st on one note; volume 0.5 | 440.0 Hz; half the level |
| the sustain pedal | holds a released note, lets go when lifted |
| 16 voices × 8 operators, 4 s | within budget at every quality (numbers below) |

**CPU per voice**, 16 voices × 8 operators (two stacks of four, feedback on top) rendered through
the engine for 4 s, per voice as a share of one core — measured on the shared build machine with
a dozen other builds running, so an upper bound:

| Quality | 1× | Auto (2× here) | 2× | 4× | 8× |
|---|---|---|---|---|---|
| % of a core per voice | 1.9–3.3 | 4.9–8.2 | 2.6–4.4 | 4.5–7.9 | 8.8–9.0 |

(Two runs each; the spread is the machine's load.) One voice alone, release build
(`modulus_speed`, same patch, same loaded machine): 1× 510–600 ns per sample (2.5–2.9 % of a
core), 2× 530 ns (2.6 %), 4× 970 ns (4.7 %), 8× 2.3 µs (10.9 %); Auto chose 1× for it. A steady note skips the per-sample ramp pass,
the operator loop visits only the operators the graph needs, and the default single-sine patch
never leaves 1×. `cargo test -p cantus-dsp --release modulus_speed -- --ignored --nocapture`
benches one voice on a quiet machine.

**Editor** (`cantus-ui/tests/inst_fm.rs`): opens in Play, Edit and Matrix and paints its name,
the OUT bar and the Carson edge; the matrix prints the tine's index; connecting, feedback and
output are one undo step each; a twenty-frame two-way drag is one undo step; an algorithm is one
step, names itself and re-applying changes nothing; SysEx banks and voices round-trip byte for
byte, clamp and report; a DX import is one step with the published scalings; Bake is one step and
the baked patch renders within 1e-4 of the unbaked one; every factory preset sets what it names in
one step and names its macros.

## Not done

- **The P1 list** (research §8.3): programming a patch from a description and explaining one
  through the AI bridge, fitting a patch to a target spectrum, ModFM and FB-saw operator modes,
  keyboard-first graph editing with AccessKit names, mutations. The model is ready for them (every
  value a key, one batch per change); none is built.
- **Per-voice modulation of the morph position**: `morph.x/y` move the whole patch (they are read
  when the patch is configured, like automation); a per-note morph from the matrix is not wired.
- **Timbre independence from the host rate with feedback**: indices are radians and envelopes are
  seconds (the test holds the centroid within 1 % at 44.1, 48 and 96 kHz), but a feedback loop's
  delay is one host sample, as on every DX, so feedback is brighter at 96 kHz. The research's
  "fixed internal rate" would need a fractional delay; the quality setting, which is what the
  research worried about, does not change it.
- **Aliasing in the half-band's transition**: content between 0.5 and 0.55·fs folds into
  0.45–0.5·fs (above 21.6 kHz at 48 kHz). The audible band is at −97 dB at index 16; the whole
  band is counted too, and published.
- **Bit-exact DX (a "Mark I" colour)**: the DX envelope follows the published formulas, not the
  chip's stair-stepped rates, 12-bit levels or 49,096 Hz clock; the SysEx mapping names its
  approximations (LFO speed, pitch-envelope times, vibrato depth, detune step).
- **Operator waves beyond four** (FM8 has 32), a noise operator and a filter node (P2).
- **Microtuning** (Scala/MTS-ESP) waits for the standards work.
