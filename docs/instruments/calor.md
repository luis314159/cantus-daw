# Calor — the analog-modelling polysynth (`InstrumentKind::Analog`)

*Owner: Calor's builder (round 17, ADR 0023). Benchmark: u-he Diva and Repro, Arturia's V
Collection, Sylenth1, Spire (`research/instruments/analog.md`). Built on the shared
virtual-analog library `cantus_dsp::va`, which Unda and Gravis may take whole.*

## What it is

A fixed, readable signal path — three oscillators, a sub and noise into a mixer whose
levels drive the filter, a high-pass, five filter topologies in one slot, the amplifier —
with two envelopes, two LFOs, the wheel, poly-mod and cross-mod as fixed routings and the
shared 32-slot matrix for everything else. What the benchmark cannot do and Calor does:

- **Reproducible analog.** Drift, oscillator phases, noise and the card a note lands on
  come from the note's id (a clip note's is stable) and a seed that is a parameter. The
  same notes give the same bits at every block size, live and rendered, and a note sounds
  the same whether the song plays from the start or from its bar (tested).
- **Sixteen voice cards you can see**, lit as they play, each with its calibration errors
  in cents, semitones and a time factor, re-rolled one at a time (one undo step).
- **One quality, live and rendered.** Draft 1× / Good 2× / Best 4×: there is no offline
  switch to forget and no host flag to trust.
- **Honest units**: cutoff in Hz and as a note, resonance in % with 100 % the marked
  self-oscillation threshold, the bass a four-pole loses in dB, drift in cents, times in
  ms, the level going into the filter in dBFS.

## Files

| Layer | File |
|---|---|
| Shared VA library | `crates/cantus-dsp/src/va.rs` and `va_math.rs` (our own `tanh`, `tan`, `exp2`, `sin`, round/floor, the counter hash), `va_filter.rs` + `va_filter_models.rs` (the five ZDF models), `va_osc.rs` + `va_osc_core.rs` (BLEP/BLAMP line, oscillator, sub), `va_env.rs`, `va_drift.rs` (drift, cards), `va_response.rs` (linear responses for editors), `va_testkit.rs`, `va_tests.rs`, `va_tests_osc.rs` |
| Parameters | `crates/cantus-core/src/inst/analog.rs` |
| DSP | `crates/cantus-dsp/src/calor.rs` and `calor_settings.rs`, `calor_synth.rs` (the instrument), `calor_notes.rs` (allocation, cards, ghosts, controllers), `calor_render.rs` (render loop, telemetry), `calor_voice.rs` + `calor_voice_run.rs` + `calor_voice_tick.rs` (a voice), `calor_chorus.rs` (chorus, output stage), `calor_tests.rs` |
| Engine | `crates/cantus-engine/src/inst/analog.rs` (spec → settings, matrix resolution, the unit) |
| Editor | `crates/cantus-ui/src/views/inst/analog.rs` (Play), `analog_filter.rs` (the display), `analog_cards.rs`, `analog_draw.rs` (the cards' small displays), `analog_edit.rs`, `analog_parts.rs` (the bound kit parts, shared with Gravis), `analog_presets.rs` |
| Strings | `crates/cantus-ui/locales/inst/analog.{en,es}.txt` |
| Tests | the DSP modules' own, `crates/cantus-engine/tests/inst_analog.rs`, `crates/cantus-ui/tests/inst_analog.rs` |

No shared file was edited.

**Screenshots** (local, git-ignored): `research/instruments/screenshots/calor/` —
`analog-light.png` and `analog-dark.png` (the Play view in the editor window, from
`cargo run -p cantus-ui --example shoot -- <dir> 1 analog`), `analog-edit-light.png` and
`analog-edit-dark.png` (the whole Edit view, from `CALOR_SHOTS=<absolute dir> cargo test -p
cantus-ui --test inst_analog the_edit_view`).

## The shared library, `cantus_dsp::va`

**Filters** (`va::filter`). Five topologies, each a trapezoidal (TPT) model whose feedback
loop is solved for the current sample, with its saturators linearised around an operating
point (`tanh(v) ≈ v · tanh(v̂)/v̂`) and the linear system solved exactly — the "cheap
non-linear zero-delay" method (Zavalishin; Voipio):

| Model | Topology | Loop gain at resonance 1.0 |
|---|---|---|
| Ladder | four transistor stages, `ẏᵢ = ω(tanh inᵢ − tanh yᵢ)`, taps 6/12/18/24 dB | `k = 4` |
| Cascade | four OTA integrators, `ẏᵢ = ω·tanh(inᵢ − yᵢ)` | `k = 4` |
| Diode | coupled diode ladder, tridiagonal (Thomas algorithm + one scalar to close the loop) | `k = 901/49`, integrators at `ω/1.1952` |
| State-variable | Simper's SVF, damping raised by the band-pass amplitude (`k = k₀ + nl·bp²`); LP → notch/BP → HP | damping 0 |
| Sallen-Key | two one-poles, band-pass difference fed back through a diode limiter | `K = 2` |

- **Every model self-oscillates at resonance 1.0 exactly on its cutoff**: each linear
  prototype was solved for where its loop reaches −180° at unit gain (for the diode ladder,
  `D(s) = s⁴ + 7s³ + 15s² + 10s + 1`, `Im D(jω) = 0` at `ω² = 10/7`), and the model scaled so
  that point is the cutoff shown.
- **The pitch tracks at every level.** Saturation pulls a stage's pole down as the
  oscillation grows (measured before the fix: the ladder 20 cents flat at resonance 1.05,
  the cascade 280). Each filter tracks the mean pole factor its stages had lately (a
  one-pole a few cycles of the cutoff long) and trims its tuning back by it, times a
  per-model factor measured once (1.5 ladder / SVF / Sallen-Key, 1.45 cascade, 1.38 diode):
  what a calibrated circuit does. The trim is weighted by resonance (none below 50 %, all
  from 100 %), so a driven filter at low resonance still darkens as it should.
- **Quality** is how hard the loop is solved: once, twice, or until the output moves less
  than 10⁻⁶ (at most eight passes; noise at full resonance at 4× averages 2.1 passes, never
  more than 3). Each sample's solve starts from the last two answers extended, which made
  one pass three times as accurate.
- Our own `tanh` ([5/4] Padé), `tan` ([5/4] Padé with the complement), `exp2`, `sin`,
  round and floor: the same bits on every platform, and faster — the platform's `roundf`
  cost 25 ns here, more than a filter stage, and made `exp2` 46 ns (now 12.5).

**Oscillators** (`va::osc`). Saw, pulse and triangle at once from one `f64` phase. Every
discontinuity is found at its fractional time and corrected with a BLEP (steps) or a
BLAMP (corners), including hard-sync resets and the saw's curvature and the triangle's
rounding in the vintage revisions. Corrections reach two samples back, so a voice's
generators share one `Blep` line with a constant two-sample latency. Two kernels: the
classic two-point polynomial and a four-point integrated cubic B-spline (whose residual
falls twice as fast in dB per octave); Calor uses the four-point everywhere. The pulse is
DC-free at any width. A sub-oscillator divides oscillator 1's wraps (square −1/−2 octaves,
25 % pulse −1).

**Envelopes** (`va::env`): RC (charging towards 1.3, discharging to −60 dB of the step so a
stage's time is its time to −60 dB), ADS, Digital, One-shot; a retrigger starts from where
the envelope is; a release lands on exact zero.

**Drift and cards** (`va::drift`): value noise hashed from (seed, octave, step), three
octaves summed; `card_trims(seed, card, roll, variance)` — the same function the DSP plays
and the editor draws.

## The instrument

**Voice path** (per voice, at 1×, 2× or 4×): three oscillators, the sub and noise into one
BLEP line → feedback from the amplifier (one sample late at the oversampled rate, 10 µs at
2×) → AC coupling if any oscillator is a vintage revision → the mixer's drive → **a 22 kHz
two-pole "circuit bandwidth" when oversampled** → high-pass (pre) → the filter, its cutoff
moved every sample by envelope 1, filter FM and poly-mod → high-pass post or post + 6 dB
shelf under 150 Hz → the amplifier (envelope 2 or the gate, velocity, drive) → pan. At
resonance ≥ 0.95 a seeded −80 dBFS noise floor enters the filter so a filter left alone
starts oscillating on its own.

*Why the bandwidth stage*: with the filter open, the saw's partials between 20 and 48 kHz
went into the input saturation and folded back (C7 at Good: −65 dB). Removing them first
put the floor at −85 dB (Best: −72 → −100 dB) for a −1 dB change at 16 kHz.

**Controls** update every 16 base samples counted from the voice's own start (so a note is
the same wherever it starts) and ramp linearly in between; drift, LFOs, glide, the matrix,
pitch, pulse widths, levels, the cutoff's base and the pan are control-rate; envelopes,
cross-mod, poly-mod and filter FM are per sample.

**Allocation**: the shared `voices::Allocator` (poly with oldest-released-first stealing,
mono/legato with priority, the pedal). A note of a unison stack takes `stack` physical
voices (at most 32 at once). Its **card**: Rotate starts at a hash of the note id and walks
to the first card not sounding (round-robin without a counter that forgets on a seek; live
input, whose ids repeat per key, rotates a counter); Reuse is the key's card; Lowest the
slot's. **A stolen voice is not cut**: it moves to one of eight ghosts that fades it over
3 ms while the new note starts fresh. A mono voice retriggers from where it is (unless the
envelopes are set to restart from silence, which ghosts it too).

**Instance**: Σ voices → one half-band decimator per channel (Comprimo's) → DC block (5 Hz)
→ chorus (one line, two taps swept by opposite triangles, I 0.52 Hz / II 0.86 Hz / I+II
8 Hz, a darker wet path) → gain → a knee that is the identity to −6 dBFS and never reaches
0 dBFS. **Latency** 2 samples at Draft, 17 at Good, 19 at Best (reported, not compensated —
ADR 0023).

**Levels**: one note at the defaults peaks at −13.5 dBFS; an eight-note chord at full
velocity at −3.1 dBFS.

## The parameters

Sparse; a `%` is stored as a fraction (the knobs print ×100); an enum is its index.

| Group | Keys |
|---|---|
| Voices | `voice.mode` (poly · mono · legato), `voice.count` 1–16, `voice.stack` 1–8, `voice.stack_detune` ct, `voice.stack_spread` %, `voice.assign` (rotate · reuse · lowest), `voice.priority` (last · low · high), `glide.time` s (minimum = off), `glide.mode` (time · per octave), `glide.curve` (RC · linear), `glide.legato`, `tune.transpose` st, `tune.fine` ct, `bend.up` / `bend.down` st |
| Oscillators | `osc{1,2,3}.octave`, `.semi` st, `.fine` ct, `.saw` / `.pulse` / `.tri` %, `.pw` 5–95 %, `.pwm` % of LFO 2, `.rev` (ideal · vintage A · vintage B); `osc{2,3}.sync`; `osc3.low`, `osc3.kbd`; `xmod.amount` %; `polymod.env` ±%, `polymod.osc3` %, `polymod.to_freq` / `.to_pw` / `.to_vcf`; `sub.shape`; `noise.color` |
| Mixer | `mix.osc1…3`, `mix.sub`, `mix.noise`, `mix.feedback` %, `mix.drive` dB |
| Filters | `hpf.cutoff` Hz (10 = off), `hpf.pos` (pre · post · boost), `vcf.model` (ladder · cascade · diode · state-variable · Sallen-Key), `vcf.slope` (poles 1–4), `vcf.morph` %, `vcf.mid` (notch · band), `vcf.cutoff` Hz, `vcf.reso` 0–120 % (100 % = self-oscillation), `vcf.drive` dB, `vcf.bass_comp` %, `vcf.key` % (100 % = 1 V/oct around C4), `vcf.env` ±oct, `vcf.vel` %, `vcf.lfo` ±oct, `vcf.fm` %, `vcf.rev` (clean · rough · pushed) |
| Amplifier | `vca.source` (env 2 · gate), `vca.vel` %, `vca.drive` %, `vca.pan` % |
| Envelopes | `env{1,2}.a` / `.d` / `.r` ms, `.s` %, `.model` (RC · ADS · digital · one-shot), `.kbd` %, `.vel` %, `.retrig` (from where it is · from silence) |
| LFOs, wheel | `lfo{1,2}.rate` Hz, `.shape` (7), `.delay` s, `.restart` (free · each note · random phase), `.phase` °, `.unipolar`; `lfo1.pitch` ct; `wheel.vibrato` ct, `wheel.mix` %, `wheel.to_pw` %, `wheel.to_vcf` ±oct |
| Analog | `analog.condition` % (Calibrated · Settled · Worn · Weathered · Unruly; 50 % applies the amounts as set), `analog.seed`, `analog.drift` ct, `analog.drift_rate` Hz, `analog.detune` ct, `analog.var_cutoff` st, `analog.var_env` %, `analog.var_pw` %, `analog.var_glide` %, `analog.tracking` ct/oct, `analog.phase` (free · reset); `card{1…16}.roll` (accepted, not listed) |
| Output | `quality` (draft · good · best), `chorus.mode` (off · I · II · I+II), `chorus.mix` %, `gain` dB |

Plus the shared model: `macro1…8`, `mod1…32.*`. Matrix targets are resolved to Calor's
own (pitch, per-oscillator pitch, widths, levels, drive, cutoff, resonance, filter drive,
morph, FM, envelope amount, bass compensation, high-pass, amplifier drive, pan, gain, LFO
rates, cross-mod, poly-mod, every envelope stage, vibrato), a depth of 1 being the
target's whole range (in octaves for a logarithmic one). Sources: `env1`, `env2`, `lfo1`,
`lfo2`, `vel`, `relvel`, `key`, `rnd`, `mpe.bend`, `mpe.press`, `mpe.slide`, the macros,
`mw`, `at`, `pb`, `expr`, `sus`.

**Per-note expression** (the contract's seven): tuning (semitones), pressure, brightness
(both sources), pan, volume, vibrato (adds to the wheel's depth), expression. They reach
their own note only and freeze after the note-off.

## The editor

On the kit's device shell (ADR 0024 §8): every section is a kit card in its function hue
(Source for the oscillators and the mixer, Tone for the filters, Dynamics for the
envelopes and the amplifier, Mod for the LFOs and the wheel, Space for the chorus, Output
for the voices, the cards and the level), every number a kit cell (value pill, knob, label
abbreviated through `<key>.short` when it does not fit), every switch a segmented control
or an LED button, every word in a box. The parts are bound in `analog_parts.rs` (shared
with Gravis): a card's controls are measured before it is painted, so a card is as tall as
what it holds and wraps when it is narrow.

- **Play**: the display first — the **filter card** (its model as a segmented control in
  the title bar; the response the DSP computes, filled; the handle on the curve at the
  cutoff, with its value, note and resonance in a pill; "self-oscillating" with a mark at
  and past 100 %; the bass a four-pole loses; the newest voice's live cutoff as a ring; the
  level into the filter as the kit's meter) — drag the curve across for the cutoff and up
  for the resonance (Shift fine, one undo step), click to put the cutoff there,
  double-click to reset; under it the cells you play it with and its slope (or the morph's
  middle) and circuit. Beside it the **voice cards** card: the sixteen cards (4 × 4) lit by
  their amplifier's envelope in the hue, with the key each plays and three bars of its
  oscillators' tuning offsets, then the Condition with its named stop and the seed, Re-roll
  all and Calibrate. Under them the amp envelope, the voice (glide, unison), the chorus and
  the output (quality as a segmented control, the CPU per voice as measured, the level).
- **Edit**: every section in signal order, card by card, scrolling past the window: the
  three oscillators (each with its cycle drawn from its levels and width, its revision in
  the title bar, sync, low range and keys as LED buttons), the mixer, the high-pass, the
  amplifier, the filter (the display again, every cell), the two envelopes (their shape
  drawn from their times on a √t axis, the live level beside it), the two LFOs (their shape
  drawn, chosen with ‹ › in the title bar), the wheel and poly-mod, the voices, the chorus,
  the output, and the voice cards (8 × 2 with their cutoff, envelope and pan trims; click
  for the trims in the help line, double-click or the button to re-roll a card, Restore,
  Re-roll all, Calibrate) with every variance cell.
- Colours are the theme's tokens only; no replica: no wood, no chrome caps, topology names.

## Presets

Twelve, each a real sound, each naming its macros (a macro is a connection: `macro1` →
a target, with a depth): Init (calibrated; Brightness, Resonance), Warm Brass
(Brightness, Bite), Round Bass (Tone, Snap), Squelch Line (Squelch, Cutoff — diode,
pushed), Glass Pluck (Tone, Length — cascade), Velvet Pad (Air, Motion — state-variable,
unison 3, chorus II), Sync Lead (Sync, Brightness; envelope 1 sweeps the synced pitch),
Poly Sweep (Poly-mod, Tone), Unison Saw (Brightness, Detune — unison 6), Screamer (Scream,
Drive — Sallen-Key), Filter Whistle (Tune, Vibrato — a self-oscillating filter played from
the keys), String Ensemble (Brightness, Ensemble — PWM, chorus I+II). Tags: init, brass,
keys, bass, lead, pluck, pad, fx, strings. Named for the sound; no hardware, song or
artist names.

## What the tests hold (numbers printed with `--nocapture`)

| Test | Holds |
|---|---|
| `va::tests::every_model_self_oscillates_on_its_cutoff` | resonance 1.05, every model, 50 Hz–10 kHz: **worst 4.9 cents at Good** (2×, two passes), **6.4 at Best** (4×, converged), 7.3 at Draft to 2.5 kHz (1×, one pass); amplitude bounded |
| `va::tests::at_the_threshold_the_ladder_rings_exactly_on_its_cutoff` | resonance 1.0, 30 Hz–12 kHz at 96 kHz: within 3 cents |
| `va::tests::resonance_costs_the_four_poles_their_bass_and_compensation_restores_it` | the ladder's gain at 100 Hz follows −20·log₁₀(1 + k) within 0.3 dB; 100 % compensation within 0.5 dB; the SVF loses none (±0.2 dB) |
| `va::tests::the_small_signal_response_matches_the_drawn_one` | eight settings, seven frequencies: the filters against `va::response` (what the editor draws) within 0.1 dB |
| `va::tests::filters_stay_finite_and_bounded_under_random_modulation` | 200 000 samples of random cutoff, resonance to 1.2, drive to +24 dB, every model and revision: finite, under +24 dBFS |
| `va::tests::distortion_rises_with_level` | THD of the ladder never falls as the level rises −40 → +12 dBFS |
| `va::tests::converging_takes_few_passes_and_keeps_its_cap` | noise at full resonance at 4×: mean 2.13 passes, never over 3 (cap 8) |
| `va::tests::our_own_functions_match_the_library_ones` | tanh 1.4·10⁻³, tan 2.7·10⁻⁷ relative, exp2 1.6·10⁻⁷ relative, sin 2.2·10⁻⁷ |
| `va::tests_osc::the_aliasing_floor_by_kernel_and_rate` | saw, 10 % pulse, triangle at 1.2/4.3/7.9 kHz through the decimator (below) |
| `va::tests_osc::hard_sync_and_cross_mod_stay_clean` | a slave at 2.37× a 220 Hz master, at 4×: floor −105.8 dB |
| `va::tests_osc::*` | a saw on pitch within 0.01 cent; a pulse DC-free at any width; the sub divides by 2 and 4; drift a pure function of seed and time; a re-roll moves one card; the RC attack on time within 5 %; a release lands on exact zero and a retrigger never steps |
| `calor::tests::every_block_size_renders_the_same_bits` | a busy patch (drift, glide, unison, sync, cross-mod, filter FM, chorus, a settings change mid-note), Draft/Good/Best, blocks 1, 7, 64, 256, 4096 and random: identical bits |
| `calor::tests::a_note_sounds_the_same_wherever_playback_starts` | a note after another has died away, and the same note from a fresh start: identical bits |
| `calor::tests::a_note_is_on_pitch_across_the_keyboard` | keys 24–108 at every quality: within 0.05 cent |
| `calor::tests::a_stolen_voice_fades_instead_of_clicking` | stealing a voice steps no more than the note itself does |
| `calor::tests::*` | exact silence and `false` once the notes end; eight notes at full velocity under 0 dBFS (−3.1 dBFS); Rotate lands six repeats of a key on six cards, Reuse on one; legato keeps the envelope; a per-note bend reaches only its note; a self-oscillating filter plays the keys within 10 cents (ladder, SVF); 4× the level into the filter gives < 4× the output; CPU per voice |
| `inst_analog` (engine) | the same bits at every block size and quality through the renderer; the same note rendered from beat 8; **every one of the 130 parameters stored at its default changes no sample**, and a route at depth 0 none; keys 21–105 within 0.1 cent; the C7 aliasing floor (below); the pedal; per-note expression; a macro route moves its target; the telemetry the editor reads; eight voices faster than real time |
| `inst_analog` (UI) | Play, Edit, Matrix paint what they are; nothing untranslated in either locale, every label and preset in both; a filter drag and a knob gesture are one undo step each; every preset sets what it names, names macro 1 and drives something with it, and is one undo step; re-rolling card 5 moves card 5 alone and is one undo step; a drag on a cell is one undo step; no word elided, cut or over another at 75 % and 200 % in English and Spanish (Play in the window, Edit whole; the ladder, the state-variable and the Sallen-Key); the Edit view renders |
| `inst_contract`, `inst_frame` | the family's floor for every kind, Calor making sound |

## Measured numbers

**Aliasing floor** (largest non-harmonic component 20 Hz–20 kHz against the fundamental,
48 kHz):

| | saw 1.2 / 4.3 / 7.9 kHz | 10 % pulse | triangle |
|---|---|---|---|
| two-point kernel, 1× | −38.6 / −29.8 / −26.4 | −30.1 / −27.2 / −26.5 | −65.8 / −46.7 / −55.2 |
| **Draft** (four-point, 1×) | −50.0 / −43.0 / −40.5 | −41.7 / −40.3 / −40.7 | −77.3 / −59.9 / −82.3 |
| **Good** (four-point, 2×) | −85.4 / −77.2 / −75.3 | −79.7 / −71.3 / −70.2 | −124 / −114 / −121 |
| **Best** (four-point, 4×) | −113 / −102 / −102 | −92.0 / −92.4 / −91.9 | −140 / −119 / −137 |

Through the whole instrument, C7 saw, filter open: at a low mixer level −95.5 dB; at the
default level (the saturation making partials of its own) Draft −50.6, **Good −84.7,
Best −99.7 dB**. The research's targets were Draft ≤ −60, Good ≤ −80, Best ≤ −100: Good is
met through the instrument and for saws and triangles alone, Best to within a dB; Draft
is not (a 1× oscillator at 7.9 kHz cannot be), and the narrow pulse at Good/Best is 8–10 dB
short (a 10 % pulse has two edges per cycle close together).

**Self-oscillation pitch error** (resonance 1.05, 50 Hz–10 kHz, every model): Good 4.9
cents worst, Best 6.4, Draft 7.3 (to 2.5 kHz). At resonance 1.0 the ladder is within
0.2 cents everywhere.

**CPU per voice** (the default patch — two oscillators into the ladder, one core, release
build, a shared machine; ranges over repeated runs): Draft 285–375 ns per voice per sample
(1.4–1.8 % of a core at 48 kHz), **Good 700–830 ns (3.4–4.0 %)**, Best 1.2–1.8 µs
(5.8–8.5 %); with the state-variable filter about 40 % less. A busy patch (three
oscillators with sync, sub, noise, filter FM, cross-mod, resonance 0.7, chorus;
`calor::tests::cpu_per_voice_at_each_quality`, test build, machine busy with other
builds): Draft 507 ns (2.4 %), Good 1.25 µs (6.0 %), Best 2.48 µs (11.9 %). Through the
engine, eight voices stacked two deep render 4.5 s of audio in 2.8 s in a test build. The research's estimates
were 35 / 130 / 340 ns: Calor spends that and more on the four-point BLEP, two solver
passes per sample, a per-sample cutoff (`tan`, `exp2`) and the 22 kHz bandwidth stage —
what buys the aliasing and tracking numbers above. The footer shows the measured figure.

## What is not done, and why

- **Switch automation with a crossfade** (P1.2): a model or slope change is immediate (the
  states carry over); no 5 ms crossfade yet.
- **LFO host sync**: the unit is not told the tempo (`configure` has the spec only); rates
  are in Hz. Needs a tempo in the contract.
- **A per-track seed** from the project seed and track id: the unit does not know its
  track. Two duplicated tracks play clip notes with different ids, so they do not flange,
  but their cards are the same until one is re-rolled.
- **"Best when rendering"** is deliberately absent: the render uses the quality you hear.
- **Pitch bend per MPE channel**: per-note bends arrive as note expressions (the contract's
  way); channel-wide bend is one value.
- **Microtuning, chain presets with inserts, channel-rack parameter locks, duo/paraphonic,
  a Behaviours list, modulation across the plug-in boundary**: P1/P2 items that need host
  work outside Calor's files.
- **CPU**: above the research's estimates (see above). A SIMD voice loop is the obvious
  next step.
- Instrument latency (2 / 17 / 19 samples) is reported, not compensated (ADR 0023).
