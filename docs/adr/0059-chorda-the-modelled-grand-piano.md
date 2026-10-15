# ADR 0059 — Chorda: the modelled grand piano

**Status:** Accepted · **Date:** 2026-10-15 · **Extends:** ADR 0023 (native instruments),
ADR 0024 (the kit shell), ADR 0031 (exact rest), ADR 0038 (voices across the pool)

## Context

BACKLOG §3e asks for a grand piano that is played from a physical model, not from
samples — the instrument a free DAW cannot otherwise offer (Pianoteq is the only
commercial one of note) — with a measurable *done when*: every partial within 1 cent of
the stretched tuning on every key; each partial's T60 within −25 % / +40 % of published
measurements (the perceptual tolerance of decay, Tolonen & Järveläinen via Bank et al.
2003); a continuous half pedal; 64 voices with the pedal down inside 25 % of one core at
256-frame blocks; and the family's rules (a default changes no sample, block-size
independence, offline equal to real time). Three patents constrain the structure: no
modal bank whose per-partial coefficients come from a soundboard parameter (Modartt US
7,915,515), no detuned non-speaking delay-loop resonators (Casio US 8,178,772), no loop
at a non-speaking pitch (Yamaha US 10,818,278). The decision to build it this round, and
why it could be finished, is `docs/round28/sound.md` §B, written before any code.

## Decisions

### 1. The keyboard's numbers are published physics, computed, never fitted to a board

`chorda_tables.rs`: per key the first partial, the inharmonicity `B`, the string count
(one to F♯1 — MIDI 30 —, two to F2 — MIDI 41 —, three above), the speaking length, wave speed and
impedance, the decay law `σ_k = b₁ + b₂ (kπ/L)²`, the hammer (mass, `K`, `p`, strike
point) and whether it has a damper (none above E6, MIDI 88).

- **Inharmonicity and tuning:** Rigaud, David & Daudet (DAFx 2011) — `B` as the sum of a
  treble and a bass exponential (`s_T = 9.26e-2, y_T = −13.64`; the bass branch
  `s_B = −0.09, y_B = −6.38` fitted to Bank's measured C2, `B = 1e-4`) — and their aural
  tuning: octaves widened to match partials of order `ρ(m) = (K−1)/2·(1 − erf((m−64)/24))
  + 1`, `K = 4.51`, from A4 outwards. A0 lands at −24 cents, C8 at +35 cents: a Railsback
  curve. *Stretch* scales the octave type's excess, *stiffness* scales `B`.
- **Strings:** Bensa et al. 2003 (C2: L 1.23 m, b₁ 0.25, b₂ 7.5e-5; C4: 0.63, 1.1, 2.7e-4;
  C7: 0.10, 9.17, 2.1e-3), interpolated on a log scale between them; the tension 700 N.
- **Hammers:** Chaigne & Askenfelt 1994 (C2 4.9 g, `K` 4e8, `p` 2.3, struck at 0.12; C4
  2.97 g, 4.5e9, 2.5, 0.12; C7 2.2 g, 1e12, 3.0, 1/16); the felt's memory after Stulov 1995
  (`ε` 0.9, `τ` 10 µs, `Q₀ = K/(1−ε)`), solved against the strings' impedance with four
  Newton steps a sample.

### 2. A string is a waveguide whose dispersion is designed, then measured

`chorda_design.rs`, `chorda_string.rs`: two rails split at the strike point; a linear-phase
three-tap loss filter (its decay follows `σ_k` and never moves a pitch); a first-order
fractional allpass; and the stiffness as **second-order allpass sections** constructed after
Abel, Välimäki & Smith (2010) and refined by Levenberg–Marquardt with an analytic Jacobian
until every partial below 5 kHz (at most 32) lies within a cent of `n F₀ √(1 + B n²)`
(the fundamental weighted ten times, then put exactly in tune by a Newton step on the
fractional delay). The fit is checked to 10 kHz. 1 to 22 sections a key (the bass needs the
most). A keyboard (88 designs and 88 reduced ones for the sympathetic strings) takes about
two seconds on eight threads.

The unisons are detuned by a hashed spread (1 cent by default) and meet at a **resistive
bridge** `Y·Z = tanh(κ·b₁/(2 f₁))`: their in-phase motion drains through it faster than
their out-of-phase motion — Weinreich's two-stage decay (measured: 24.6 dB/s prompt, 8.7
dB/s aftersound at middle C). **Phantom partials** come from the tension's modulation
(`∝ v²` of the strings' arriving waves) through a longitudinal-mode resonator.

### 3. The post-voice stage: sympathetic strings at the keys' own pitches, a multirate board

`chorda_board.rs`, after the voices' lanes are summed in key order (ADR 0038):

- **Sympathetic strings**: every key's own string, at its speaking pitch (a reduced design
  of that key, at most two dispersion sections: the first partial exact, the low ones
  close), driven one way by the bridge's motion (`0.2 · Y · F`), its damper the key's —
  lifted by the pedal continuously, or skipped while the key's own voice sounds; the keys
  above E6 ring always. Never a detuned or non-speaking loop (Casio, Yamaha).
- **The soundboard** after Bank et al. 2003: 40 low modes at a quarter of the rate (60 Hz –
  1.5 kHz, loss factor ≈ 3 %), a four-line Hadamard FDN for the high band whose decay
  shortens with frequency, and the direct radiation (a 35 Hz high-pass, a tilt). A filter
  after the strings, never a source of their coefficients (Modartt).
- **Radiation**: a plate below coincidence radiates better as frequency rises; the output
  rises 20 dB between 120 Hz and 1.2 kHz against the bridge's force — what levels the
  keyboard (A0 −10.7, C4 −9.1, C6 −13.6 dBFS peak at full velocity, alone).

### 4. Dampers and pedals

A released key's damper grips with a contact `1 − smoothstep(0.15 … 0.65)` of the pedal
(the half-pedal zone) times the *dampers* setting, gliding over 8 ms: no step between two
models, so re-pedalling leaves no discontinuity. The soft pedal (CC 67) shifts the next
strikes' hammers off one string onto softer felt (*soft pedal* sets how far).

### 5. Every clock counts from the stream, so any split gives the same bits

The voices' control steps (32 samples) count from their strike; the board's quarter-rate
steps and the chunks (64 samples) from the instrument's start. A chunk ends only where the
stream's grid does; the sympathetic dampers learn which keys sound only on that grid; the
sympathetic strings wake on drive *seen*, never on drive looked ahead (a block's end would
cut that look); the board's tail is flushed below 1e-10 only on the grid. `skip` moves every
clock as processing would. (The instrument contract caught the first version's chunks
counted from each block.)

### 6. Designs are computed off the audio thread and looked up lock-free

A registry of 32 entries (`OnceLock`s, keyed by A4, stretch, stiffness and the rate): the
snapshot's tunings are requested when its units are built (`units_for`), the reference
keyboard is requested at start-up (`assemble`) and after a device change. A real-time unit
plays its last keyboard until the new one is ready (the editor says *designing the
strings*); an offline render waits for its own (at most two minutes). `a4`, `stretch` and
`stiffness` are set, never modulated or automated (a route to them is ignored).

### 7. The model is versioned

`model` (1) is stored with every instance; a later model never changes a song made with
this one.

## Consequences

- A free, sample-less grand piano with an editor that shows the physics — strings at their
  lengths, the half-pedal law, the tuning's stretch, the felt, the ring — in Play / Edit /
  Matrix, ten presets with four macros, both languages, the ballad demo.
- 88 voices (one per key: a repeated note strikes the strings that still ring), every
  delay line allocated when the unit is built, never on the audio thread.
- The window pauses a few seconds once when Chorda is first loaded at a rate it has not
  designed (the manual's known limitations).
- **The owner's, before release** (`docs/beta-checklist.md`): the blind listening test
  (≥ 20 listeners against Pianoteq 9 and a sampled grand) and the legal review of the
  structure against every claim of the patents above.

## What the tests hold

| Done when | Test | Measured |
|---|---|---|
| Every partial within 1 cent | `chorda::tests_design::every_key_is_designed_within_a_cent…` (88 keys, 44.1 and 48 kHz); `tests_string::a_string_rings_on_the_stiff_strings_partials_within_a_cent` (rendered, phase vocoder) | worst 0.49 cents designed (48 kHz), 0.40 cents measured on the sound |
| Per-partial T60 within −25 %/+40 % of Bensa et al. | `tests_string::a_string_decays_as_bensa_et_al_measured` (C2, C4, C7) | within −4 % / +1 % |
| Half pedal continuous | `tests_piano::half_pedalling_is_continuous`, `re_pedalling_leaves_no_discontinuity` | decay 241 → 41 dB/s across the pedal, largest step 57 dB/s (28 % of the range) |
| 64 pedal-down voices < 25 % of a core at 256 frames | `tests_piano::measure_sixty_four_pedal_down_voices` (ignored, release) | 18.2–19.1 % (best of three, this PC) |
| Defaults change no sample; any split; parallel = serial; exact rest | `inst_chorda.rs`, `inst_contract.rs` (split, voices, at_rest, block sizes), `tests_engine::notes_and_pedals_inside_a_block…` | to the bit |
| Offline = real time | `inst_chorda::offline_render_equals_real_time` (reference tuning and a designed one) | to the bit |
| Headroom | `inst_chorda::its_levels_leave_headroom…`, `instrument_levels` (every preset, dense riff) | both hands ff with the pedal −3.3 dBFS; loudest preset −6 dBFS |
| Two-stage decay, una corda, sympathy | `tests_piano::*` | 24.6 / 8.7 dB/s; softer and darker; sympathy −17 dB with the pedal, −32 dB without |
| The editor | `cantus-ui/tests/inst_chorda.rs` (views, undo, drop, presets, no elided or overlapping text at 75–200 % and in Spanish, kit lint, the boot path) | — |
